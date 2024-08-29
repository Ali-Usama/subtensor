//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use crate::cli::Sealing;
use crate::ethereum::{
    db_config_dir, new_frontier_partial, spawn_frontier_tasks, BackendType, EthConfiguration,
    FrontierBackend, FrontierBlockImport, FrontierPartialComponents, StorageOverride,
    StorageOverrideHandler,
};
use futures::{channel::mpsc, future, FutureExt};
use node_subtensor_runtime::{opaque::Block, Hash, RuntimeApi, TransactionConverter};
use sc_client_api::{Backend, BlockBackend};
use sc_consensus::BasicQueue;
use sc_consensus_aura::{SlotProportion, StartAuraParams};
use sc_consensus_grandpa::SharedVoterState;
use sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging;
use sc_executor::sp_wasm_interface::{Function, HostFunctionRegistry, HostFunctions};
pub use sc_executor::NativeElseWasmExecutor;
use sc_network_sync::strategy::warp::{WarpSyncParams, WarpSyncProvider};
use sc_service::{error::Error as ServiceError, Configuration, PartialComponents, TaskManager};
use sc_telemetry::{log, Telemetry, TelemetryHandle, TelemetryWorker};
use sc_transaction_pool::FullPool;
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;
use sp_core::U256;
use std::{cell::RefCell, path::Path};
use std::{sync::Arc, time::Duration};
use substrate_prometheus_endpoint::Registry;

/// The minimum period of blocks on which justifications will be
/// imported and generated.
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

type BasicImportQueue = sc_consensus::DefaultImportQueue<Block>;
type GrandpaBlockImport<Client> =
    sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, Client, FullSelectChain>;

// Our native executor instance.
pub struct ExecutorDispatch;

// appeasing the compiler, this is a no-op
impl HostFunctions for ExecutorDispatch {
    fn host_functions() -> Vec<&'static dyn Function> {
        vec![]
    }

    fn register_static<T>(_registry: &mut T) -> core::result::Result<(), T::Error>
    where
        T: HostFunctionRegistry,
    {
        Ok(())
    }
}

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
    // Only enable the benchmarking host functions when we actually want to benchmark.
    #[cfg(feature = "runtime-benchmarks")]
    type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
    // Otherwise we only use the default Substrate host functions.
    #[cfg(not(feature = "runtime-benchmarks"))]
    type ExtendHostFunctions = ();

    fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
        node_subtensor_runtime::api::dispatch(method, data)
    }

    fn native_version() -> sc_executor::NativeVersion {
        node_subtensor_runtime::native_version()
    }
}

pub(crate) type FullClient =
    sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
pub type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type BoxBlockImport = sc_consensus::BoxBlockImport<Block>;

pub fn new_partial<BIQ>(
    config: &Configuration,
    eth_config: &EthConfiguration,
    build_import_queue: BIQ,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        BasicImportQueue,
        FullPool<Block, FullClient>,
        (
            BoxBlockImport,
            sc_consensus_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
            Option<Telemetry>,
            FrontierBackend<FullClient>,
            Arc<dyn StorageOverride<Block>>,
        ),
    >,
    ServiceError,
>
where
    BIQ: FnOnce(
        Arc<FullClient>,
        &Configuration,
        &EthConfiguration,
        &TaskManager,
        Option<TelemetryHandle>,
        GrandpaBlockImport<FullClient>,
    ) -> Result<(BasicImportQueue, BoxBlockImport), ServiceError>,
{
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_native_or_wasm_executor(config);

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
        client.clone(),
        GRANDPA_JUSTIFICATION_PERIOD,
        &(client.clone() as Arc<_>),
        select_chain.clone(),
        telemetry.as_ref().map(|x| x.handle()),
    )?;

    let (import_queue, block_import) = build_import_queue(
        client.clone(),
        config,
        eth_config,
        &task_manager,
        telemetry.as_ref().map(|x| x.handle()),
        grandpa_block_import,
    )?;

    let storage_override = Arc::new(StorageOverrideHandler::new(client.clone()));
    let frontier_backend = match eth_config.frontier_backend_type {
        BackendType::KeyValue => FrontierBackend::KeyValue(Arc::new(fc_db::kv::Backend::open(
            Arc::clone(&client),
            &config.database,
            &db_config_dir(config),
        )?)),
        BackendType::Sql => {
            let db_path = db_config_dir(config).join("sql");
            std::fs::create_dir_all(&db_path).expect("failed creating sql db directory");
            let backend = futures::executor::block_on(fc_db::sql::Backend::new(
                fc_db::sql::BackendConfig::Sqlite(fc_db::sql::SqliteBackendConfig {
                    path: Path::new("sqlite:///")
                        .join(db_path)
                        .join("frontier.db3")
                        .to_str()
                        .unwrap(),
                    create_if_missing: true,
                    thread_count: eth_config.frontier_sql_backend_thread_count,
                    cache_size: eth_config.frontier_sql_backend_cache_size,
                }),
                eth_config.frontier_sql_backend_pool_size,
                std::num::NonZeroU32::new(eth_config.frontier_sql_backend_num_ops_timeout),
                storage_override.clone(),
            ))
            .unwrap_or_else(|err| panic!("failed creating sql backend: {:?}", err));
            FrontierBackend::Sql(Arc::new(backend))
        }
    };

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (
            block_import,
            grandpa_link,
            telemetry,
            frontier_backend,
            storage_override,
        ),
    })
}

/// Build the import queue for the template runtime (aura + grandpa).
pub fn build_aura_grandpa_import_queue(
    client: Arc<FullClient>,
    config: &Configuration,
    eth_config: &EthConfiguration,
    task_manager: &TaskManager,
    telemetry: Option<TelemetryHandle>,
    grandpa_block_import: GrandpaBlockImport<FullClient>,
) -> Result<(BasicImportQueue, BoxBlockImport), ServiceError> {
    let frontier_block_import =
        FrontierBlockImport::new(grandpa_block_import.clone(), client.clone());

    let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
    let target_gas_price = eth_config.target_gas_price;
    let create_inherent_data_providers = move |_, ()| async move {
        let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
        let slot =
            sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                *timestamp,
                slot_duration,
            );
        let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
        Ok((slot, timestamp, dynamic_fee))
    };

    let import_queue = sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(
        sc_consensus_aura::ImportQueueParams {
            block_import: frontier_block_import.clone(),
            justification_import: Some(Box::new(grandpa_block_import)),
            client,
            create_inherent_data_providers,
            spawner: &task_manager.spawn_essential_handle(),
            registry: config.prometheus_registry(),
            check_for_equivocation: Default::default(),
            telemetry,
            compatibility_mode: sc_consensus_aura::CompatibilityMode::None,
        },
    )
    .map_err::<ServiceError, _>(Into::into)?;

    Ok((import_queue, Box::new(frontier_block_import)))
}

/// Build the import queue for the template runtime (manual seal).
pub fn build_manual_seal_import_queue(
    client: Arc<FullClient>,
    config: &Configuration,
    _eth_config: &EthConfiguration,
    task_manager: &TaskManager,
    _telemetry: Option<TelemetryHandle>,
    _grandpa_block_import: GrandpaBlockImport<FullClient>,
) -> Result<(BasicImportQueue, BoxBlockImport), ServiceError> {
    let frontier_block_import = FrontierBlockImport::new(client.clone(), client);
    Ok((
        sc_consensus_manual_seal::import_queue(
            Box::new(frontier_block_import.clone()),
            &task_manager.spawn_essential_handle(),
            config.prometheus_registry(),
        ),
        Box::new(frontier_block_import),
    ))
}

// Builds a new service for a full client.
pub async fn new_full<
    N: sc_network::NetworkBackend<Block, <Block as sp_runtime::traits::Block>::Hash>,
>(
    mut config: Configuration,
    eth_config: EthConfiguration,
    sealing: Option<Sealing>,
) -> Result<TaskManager, ServiceError> {
    let build_import_queue = if sealing.is_some() {
        build_manual_seal_import_queue
    } else {
        build_aura_grandpa_import_queue
    };

    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (block_import, grandpa_link, mut telemetry, frontier_backend, storage_override),
    } = new_partial(&config, &eth_config, build_import_queue)?;

    let FrontierPartialComponents {
        filter_pool,
        fee_history_cache,
        fee_history_cache_limit,
    } = new_frontier_partial(&eth_config)?;

    let mut net_config = sc_network::config::FullNetworkConfiguration::<
        Block,
        <Block as sp_runtime::traits::Block>::Hash,
        N,
    >::new(&config.network);
    let metrics = N::register_notification_metrics(config.prometheus_registry());
    let peer_store_handle = net_config.peer_store_handle();

    let grandpa_protocol_name = sc_consensus_grandpa::protocol_standard_name(
        &client
            .block_hash(0)
            .ok()
            .flatten()
            .expect("Genesis block exists; qed"),
        &config.chain_spec,
    );

    let (grandpa_protocol_config, grandpa_notification_service) =
        sc_consensus_grandpa::grandpa_peers_set_config::<_, N>(
            grandpa_protocol_name.clone(),
            metrics.clone(),
            peer_store_handle,
        );

    let warp_sync_params = if sealing.is_some() {
        None
    } else {
        net_config.add_notification_protocol(grandpa_protocol_config);
        let warp_sync: Arc<dyn WarpSyncProvider<Block>> =
            Arc::new(sc_consensus_grandpa::warp_proof::NetworkProvider::new(
                backend.clone(),
                grandpa_link.shared_authority_set().clone(),
                Vec::default(),
            ));
        Some(WarpSyncParams::WithProvider(warp_sync))
    };

    let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_params,
            block_relay: None,
            metrics,
        })?;

    if config.offchain_worker.enabled {
        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-worker",
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                is_validator: config.role.is_authority(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: Arc::new(network.clone()),
                enable_http_requests: true,
                custom_extensions: |_| vec![],
            })
            .run(client.clone(), task_manager.spawn_handle())
            .boxed(),
        );
    }

    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks = Some(BackoffAuthoringOnFinalizedHeadLagging {
        unfinalized_slack: 6,
        ..Default::default()
    });
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();
    let frontier_backend = Arc::new(frontier_backend);

    // Channel for the rpc handler to communicate with the authorship task.
    let (command_sink, commands_stream) = mpsc::channel(1000);

    // Sinks for pubsub notifications.
    // Everytime a new subscription is created, a new mpsc channel is added to the sink pool.
    // The MappingSyncWorker sends through the channel on block import and the subscription emits a notification to the subscriber on receiving a message through this channel.
    // This way we avoid race conditions when using native substrate block import notification stream.
    let pubsub_notification_sinks: fc_mapping_sync::EthereumBlockNotificationSinks<
        fc_mapping_sync::EthereumBlockNotification<Block>,
    > = Default::default();
    let pubsub_notification_sinks = Arc::new(pubsub_notification_sinks);

    // for ethereum-compatibility rpc.
    config.rpc_id_provider = Some(Box::new(fc_rpc::EthereumSubIdProvider));

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();

        let network = network.clone();
        let sync_service = sync_service.clone();

        let is_authority = role.is_authority();
        let enable_dev_signer = eth_config.enable_dev_signer;
        let max_past_logs = eth_config.max_past_logs;
        let execute_gas_limit_multiplier = eth_config.execute_gas_limit_multiplier;
        let filter_pool = filter_pool.clone();
        let frontier_backend = frontier_backend.clone();
        let pubsub_notification_sinks = pubsub_notification_sinks.clone();
        let storage_override = storage_override.clone();
        let fee_history_cache = fee_history_cache.clone();
        let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
            task_manager.spawn_handle(),
            storage_override.clone(),
            eth_config.eth_log_block_cache,
            eth_config.eth_statuses_cache,
            prometheus_registry.clone(),
        ));

        let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
        let target_gas_price = eth_config.target_gas_price;
        let pending_create_inherent_data_providers = move |_, ()| async move {
            let current = sp_timestamp::InherentDataProvider::from_system_time();
            let next_slot = current.timestamp().as_millis() + slot_duration.as_millis();
            let timestamp = sp_timestamp::InherentDataProvider::new(next_slot.into());
            let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
				*timestamp,
				slot_duration,
			);
            let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
            Ok((slot, timestamp, dynamic_fee))
        };

        Box::new(
            move |deny_unsafe, subscription_executor: sc_rpc::SubscriptionTaskExecutor| {
                let eth_deps = crate::rpc::EthDeps {
                    client: client.clone(),
                    pool: pool.clone(),
                    graph: pool.pool().clone(),
                    converter: Some(TransactionConverter),
                    is_authority,
                    enable_dev_signer,
                    network: network.clone(),
                    sync: sync_service.clone(),
                    frontier_backend: match &*frontier_backend {
                        fc_db::Backend::KeyValue(b) => b.clone(),
                        fc_db::Backend::Sql(b) => b.clone(),
                    },
                    storage_override: storage_override.clone(),
                    block_data_cache: block_data_cache.clone(),
                    filter_pool: filter_pool.clone(),
                    max_past_logs,
                    fee_history_cache: fee_history_cache.clone(),
                    fee_history_cache_limit,
                    execute_gas_limit_multiplier,
                    forced_parent_hashes: None,
                    pending_create_inherent_data_providers,
                };

                let deps = crate::rpc::FullDeps {
                    client: client.clone(),
                    pool: pool.clone(),
                    deny_unsafe,
                    command_sink: if sealing.is_some() {
                        Some(command_sink.clone())
                    } else {
                        None
                    },
                    eth: eth_deps,
                };
                crate::rpc::create_full(
                    deps,
                    subscription_executor,
                    pubsub_notification_sinks.clone(),
                )
                .map_err(Into::into)
            },
        )
    };

    let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        network: network.clone(),
        client: client.clone(),
        keystore: keystore_container.keystore(),
        task_manager: &mut task_manager,
        transaction_pool: transaction_pool.clone(),
        rpc_builder: rpc_extensions_builder,
        backend: backend.clone(),
        system_rpc_tx,
        tx_handler_controller,
        sync_service: sync_service.clone(),
        config,
        telemetry: telemetry.as_mut(),
    })?;

    spawn_frontier_tasks(
        &task_manager,
        client.clone(),
        backend,
        frontier_backend,
        filter_pool,
        storage_override,
        fee_history_cache,
        fee_history_cache_limit,
        sync_service.clone(),
        pubsub_notification_sinks,
    )
    .await;

    if role.is_authority() {
        // manual-seal authorship
        if let Some(sealing) = sealing {
            run_manual_seal_authorship(
                &eth_config,
                sealing,
                client,
                transaction_pool,
                select_chain,
                block_import,
                &task_manager,
                prometheus_registry.as_ref(),
                telemetry.as_ref(),
                commands_stream,
            )?;

            network_starter.start_network();
            log::info!("Manual Seal Ready");
            return Ok(task_manager);
        }

        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        let slot_duration = sc_consensus_aura::slot_duration(&*client)?;

        let aura = sc_consensus_aura::start_aura::<AuraPair, _, _, _, _, _, _, _, _, _, _>(
            StartAuraParams {
                slot_duration,
                client,
                select_chain,
                block_import,
                proposer_factory,
                create_inherent_data_providers: move |_, ()| async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let slot =
						sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

                    Ok((slot, timestamp))
                },
                force_authoring,
                backoff_authoring_blocks,
                keystore: keystore_container.keystore(),
                sync_oracle: sync_service.clone(),
                justification_sync_link: sync_service.clone(),
                block_proposal_slot_portion: SlotProportion::new(2f32 / 3f32),
                max_block_proposal_slot_portion: None,
                telemetry: telemetry.as_ref().map(|x| x.handle()),
                compatibility_mode: Default::default(),
            },
        )?;

        // the AURA authoring task is considered essential, i.e. if it
        // fails we take down the service with it.
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("aura", Some("block-authoring"), aura);
    }

    if enable_grandpa {
        // if the node isn't actively participating in consensus then it doesn't
        // need a keystore, regardless of which protocol we use below.
        let keystore = if role.is_authority() {
            Some(keystore_container.keystore())
        } else {
            None
        };

        let grandpa_config = sc_consensus_grandpa::Config {
            // FIXME #1578 make this available through chainspec
            gossip_duration: Duration::from_millis(333),
            justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
            name: Some(name),
            observer_enabled: false,
            keystore,
            local_role: role,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            protocol_name: grandpa_protocol_name,
        };

        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_config = sc_consensus_grandpa::GrandpaParams {
            config: grandpa_config,
            link: grandpa_link,
            network,
            voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry,
            shared_voter_state: SharedVoterState::empty(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool),
            sync: Arc::new(sync_service),
            notification_service: grandpa_notification_service,
        };

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "grandpa-voter",
            None,
            sc_consensus_grandpa::run_grandpa_voter(grandpa_config)?,
        );
    }

    network_starter.start_network();
    Ok(task_manager)
}

pub fn new_chain_ops(
    config: &mut Configuration,
    eth_config: &EthConfiguration,
) -> Result<
    (
        Arc<FullClient>,
        Arc<FullBackend>,
        BasicQueue<Block>,
        TaskManager,
        FrontierBackend<FullClient>,
    ),
    ServiceError,
> {
    config.keystore = sc_service::config::KeystoreConfig::InMemory;
    let PartialComponents {
        client,
        backend,
        import_queue,
        task_manager,
        other,
        ..
    } = new_partial::<_>(config, eth_config, build_aura_grandpa_import_queue)?;
    Ok((client, backend, import_queue, task_manager, other.3))
}

fn run_manual_seal_authorship(
    eth_config: &EthConfiguration,
    sealing: Sealing,
    client: Arc<FullClient>,
    transaction_pool: Arc<FullPool<Block, FullClient>>,
    select_chain: FullSelectChain,
    block_import: BoxBlockImport,
    task_manager: &TaskManager,
    prometheus_registry: Option<&Registry>,
    telemetry: Option<&Telemetry>,
    commands_stream: mpsc::Receiver<sc_consensus_manual_seal::rpc::EngineCommand<Hash>>,
) -> Result<(), ServiceError> {
    let proposer_factory = sc_basic_authorship::ProposerFactory::new(
        task_manager.spawn_handle(),
        client.clone(),
        transaction_pool.clone(),
        prometheus_registry,
        telemetry.as_ref().map(|x| x.handle()),
    );

    thread_local!(static TIMESTAMP: RefCell<u64> = const { RefCell::new(0) });

    /// Provide a mock duration starting at 0 in millisecond for timestamp inherent.
    /// Each call will increment timestamp by slot_duration making Aura think time has passed.
    struct MockTimestampInherentDataProvider;

    #[async_trait::async_trait]
    impl sp_inherents::InherentDataProvider for MockTimestampInherentDataProvider {
        async fn provide_inherent_data(
            &self,
            inherent_data: &mut sp_inherents::InherentData,
        ) -> Result<(), sp_inherents::Error> {
            TIMESTAMP.with(|x| {
                *x.borrow_mut() += node_subtensor_runtime::SLOT_DURATION;
                inherent_data.put_data(sp_timestamp::INHERENT_IDENTIFIER, &*x.borrow())
            })
        }

        async fn try_handle_error(
            &self,
            _identifier: &sp_inherents::InherentIdentifier,
            _error: &[u8],
        ) -> Option<Result<(), sp_inherents::Error>> {
            // The pallet never reports error.
            None
        }
    }

    let target_gas_price = eth_config.target_gas_price;
    let create_inherent_data_providers = move |_, ()| async move {
        let timestamp = MockTimestampInherentDataProvider;
        let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
        Ok((timestamp, dynamic_fee))
    };

    let manual_seal = match sealing {
        Sealing::Manual => future::Either::Left(sc_consensus_manual_seal::run_manual_seal(
            sc_consensus_manual_seal::ManualSealParams {
                block_import,
                env: proposer_factory,
                client,
                pool: transaction_pool,
                commands_stream,
                select_chain,
                consensus_data_provider: None,
                create_inherent_data_providers,
            },
        )),
        Sealing::Instant => future::Either::Right(sc_consensus_manual_seal::run_instant_seal(
            sc_consensus_manual_seal::InstantSealParams {
                block_import,
                env: proposer_factory,
                client,
                pool: transaction_pool,
                select_chain,
                consensus_data_provider: None,
                create_inherent_data_providers,
            },
        )),
    };

    // we spawn the future on a background thread managed by service.
    task_manager
        .spawn_essential_handle()
        .spawn_blocking("manual-seal", None, manual_seal);
    Ok(())
}
