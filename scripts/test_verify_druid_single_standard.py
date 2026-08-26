#!/usr/bin/env python3
"""Adversarial self-tests for the P04 Task 5 Druid single-standard verifier."""

from __future__ import annotations

import tempfile
import unittest
from collections import Counter
from pathlib import Path
from unittest import mock

import verify_druid_single_standard as verifier

from verify_druid_single_standard import (
    VerificationError,
    TASK5_CLOSED,
    TASK6_PENDING,
    TASK7_PENDING,
    classify_frozen_task5_row,
    require_concrete_object,
    require_admission_composition_source,
    require_direct_standard_impl,
    require_fixed_files,
    require_holder_contract,
    require_identity_counter_hash,
    require_owner_close_contract,
    require_shared_execution_policy,
    repository_root,
    verify_repository,
    verify_task4_evidence_handoff,
    verify_task5_runtime_contracts,
    verify_task5_consumer_states,
)


class DruidSingleStandardVerifierTest(unittest.TestCase):
    """Pressure-test every fail-closed Task 5 evidence boundary."""

    def test_rejects_alias_or_facade_instead_of_concrete_object(self) -> None:
        with self.assertRaisesRegex(VerificationError, "concrete object"):
            require_concrete_object(
                "pub use druid_rdbc::spi::PreparedStatementKey;",
                "PreparedStatementKey",
                "prepared_statement_key.rs",
            )
        with self.assertRaisesRegex(VerificationError, "concrete object"):
            require_concrete_object(
                "pub type PreparedStatementKey = druid_rdbc::spi::PreparedStatementKey;",
                "PreparedStatementKey",
                "prepared_statement_key.rs",
            )

    def test_rejects_empty_impl_and_default_success(self) -> None:
        source = """
            pub struct DruidPooledConnection { connection: usize }
            impl druid_rdbc::connection::Connection for DruidPooledConnection {
                async fn close(&mut self) -> Result<(), RdbcError> { Ok(()) }
                fn is_closed(&self) -> Result<bool, RdbcError> { Ok(false) }
            }
        """
        with self.assertRaisesRegex(VerificationError, "default-success"):
            require_direct_standard_impl(
                source,
                "druid_rdbc::connection::Connection",
                "DruidPooledConnection",
                {"close", "is_closed"},
                "druid_pooled_connection.rs",
            )

    def test_rejects_cfg_disabled_standard_identity(self) -> None:
        source = """
            pub struct DruidPooledConnection { connection: usize }
            #[cfg(any())]
            impl druid_rdbc::connection::Connection for DruidPooledConnection {
                async fn close(&mut self) -> Result<(), RdbcError> { self.recycle_once(); Ok(()) }
                fn is_closed(&self) -> Result<bool, RdbcError> { self.logical_closed() }
            }
        """
        with self.assertRaisesRegex(VerificationError, "cfg-disabled"):
            require_direct_standard_impl(
                source,
                "druid_rdbc::connection::Connection",
                "DruidPooledConnection",
                {"close", "is_closed"},
                "druid_pooled_connection.rs",
            )

    def test_rejects_hidden_physical_alias_inside_standard_impl(self) -> None:
        source = """
            use druid_rdbc::spi::PhysicalConnection as HiddenConnection;
            pub struct DruidPooledConnection { raw: Box<dyn HiddenConnection> }
            impl druid_rdbc::connection::Connection for DruidPooledConnection {
                async fn close(&mut self) -> Result<(), RdbcError> {
                    HiddenConnection::close(&mut *self.raw).await
                }
                fn is_closed(&self) -> Result<bool, RdbcError> {
                    Ok(HiddenConnection::is_closed(&*self.raw))
                }
            }
        """
        with self.assertRaisesRegex(VerificationError, "Physical"):
            require_direct_standard_impl(
                source,
                "druid_rdbc::connection::Connection",
                "DruidPooledConnection",
                {"close", "is_closed"},
                "druid_pooled_connection.rs",
            )

    def test_rejects_duplicated_standard_and_physical_core(self) -> None:
        source = """
            impl druid_rdbc::connection::Connection for DruidPooledConnection {
                async fn close(&mut self) -> Result<(), RdbcError> {
                    self.closed = true; self.callback.take(); Ok(())
                }
            }
            impl druid_rdbc::spi::PhysicalConnection for DruidPooledConnection {
                async fn close(&mut self) -> Result<(), RdbcError> {
                    self.closed = true; self.callback.take(); Ok(())
                }
            }
        """
        with self.assertRaisesRegex(VerificationError, "duplicated core"):
            require_direct_standard_impl(
                source,
                "druid_rdbc::connection::Connection",
                "DruidPooledConnection",
                {"close"},
                "druid_pooled_connection.rs",
            )

    def test_holder_requires_standard_slot_and_private_owner_policy(self) -> None:
        physical = """
            pub struct DruidConnectionHolder {
                physical_connection: Option<Box<dyn PhysicalConnection>>,
            }
        """
        with self.assertRaisesRegex(VerificationError, "Box<dyn Connection>"):
            require_holder_contract(physical, "druid_connection_holder.rs")

        public_owner = """
            pub enum ConnectionOwnerPolicy { NativePool, ExternalLease }
            pub struct DruidConnectionHolder {
                connection: Option<Box<dyn druid_rdbc::connection::Connection>>,
                owner: ConnectionOwnerPolicy,
            }
        """
        with self.assertRaisesRegex(VerificationError, "private owner policy"):
            require_holder_contract(public_owner, "druid_connection_holder.rs")

    def test_logical_close_cannot_close_raw_and_external_cannot_return_native(
        self,
    ) -> None:
        wrong = """
            enum ConnectionOwnerPolicy { NativePool, ExternalLease }
            impl DruidPooledConnection {
                async fn close_logical(&mut self) -> Result<(), RdbcError> {
                    self.holder.close_standard_connection().await
                }
                fn return_external(&mut self) { self.native_pool.push(self.holder.take()); }
            }
        """
        with self.assertRaisesRegex(VerificationError, "logical/raw close ownership"):
            require_owner_close_contract(wrong, "druid_pooled_connection.rs")

    def test_fixed_file_omission_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "crates/druid/src/core").mkdir(parents=True)
            with self.assertRaisesRegex(VerificationError, "required Task 5 file"):
                require_fixed_files(root)

    def test_consumer_states_cover_all_dynamic_task5_rows(self) -> None:
        manifest = """source_path\toccurrence_kind\tsymbol\told_path\ttarget_path\ttarget_task\tstate
crates/druid/src/core/a.rs\tTYPE\tA@token=1\tdruid_rdbc::spi::A\tdruid::core::a\tP04_TASK_5_DRUID_INTERNALIZATION\tDRUID_INTERNAL_REFERENCE_CLOSED
crates/druid/src/core/b.rs\tTYPE\tB@token=2\tdruid_rdbc::spi::B\tdruid::core::b\tP04_TASK_5_DRUID_INTERNALIZATION\tDRUID_INTERNAL_REFERENCE_CLOSED
"""
        verify_task5_consumer_states(manifest.encode(), expected_rows=2)

    def test_consumer_states_reject_planned_unknown_or_missing_rows(self) -> None:
        planned = """source_path\toccurrence_kind\tsymbol\told_path\ttarget_path\ttarget_task\tstate
crates/druid/src/core/a.rs\tTYPE\tA@token=1\tdruid_rdbc::spi::A\tdruid::core::a\tP04_TASK_5_DRUID_INTERNALIZATION\tCURRENT_REFERENCE_TARGET_PLANNED
"""
        with self.assertRaisesRegex(VerificationError, "Task 5 consumer state"):
            verify_task5_consumer_states(planned.encode(), expected_rows=1)
        closed = planned.replace(
            "CURRENT_REFERENCE_TARGET_PLANNED", "DRUID_INTERNAL_REFERENCE_CLOSED"
        )
        with self.assertRaisesRegex(VerificationError, "dynamic Task 5 denominator"):
            verify_task5_consumer_states(closed.encode(), expected_rows=2)

    def test_consumer_states_reject_obsolete_task5_live_state(self) -> None:
        """No migrated or pending Task 5 identity may be mislabeled as generically LIVE."""

        obsolete_live = """source_path\toccurrence_kind\tsymbol\told_path\ttarget_path\ttarget_task\tstate
crates/druid/src/core/a.rs\tTYPE\tA@token=1\tdruid_rdbc::spi::A\tdruid::core::a\tP04_TASK_5_DRUID_INTERNALIZATION\tDRUID_INTERNAL_REFERENCE_LIVE
"""
        with self.assertRaisesRegex(VerificationError, "unknown state"):
            verify_task5_consumer_states(obsolete_live.encode(), expected_rows=1)

    def test_frozen_classifier_routes_task5_task6_and_task7_exactly(self) -> None:
        from verify_rdbc_standardization_denominator import ConsumerRow

        def row(path: str, old_path: str) -> ConsumerRow:
            return ConsumerRow(
                path,
                "TYPE",
                old_path.rsplit("::", 1)[-1] + "@token=1",
                old_path,
                "druid::core::target",
                "P04_TASK_5_DRUID_INTERNALIZATION",
                "CURRENT_REFERENCE_TARGET_PLANNED",
            )

        self.assertEqual(
            classify_frozen_task5_row(
                row(
                    "crates/druid-rdbc/src/resource/blob.rs",
                    "druid_rdbc::resource::RdbcResourceContext",
                )
            ),
            TASK5_CLOSED,
        )
        self.assertEqual(
            classify_frozen_task5_row(
                row(
                    "crates/druid-rdbc/src/spi/physical_connection.rs",
                    "druid_rdbc::spi::PreparedStatementKey",
                )
            ),
            TASK6_PENDING,
        )
        self.assertEqual(
            classify_frozen_task5_row(
                row(
                    "crates/druid-rdbc/src/resource/blob.rs",
                    "druid_rdbc::resource::RdbcResourceCapabilities",
                )
            ),
            TASK7_PENDING,
        )

    def test_identity_hash_rejects_fabricated_compatibility_addition(self) -> None:
        """Compatibility additions are an exact multiset, not merely an allowed directory."""

        identity = (
            "crates/druid/src/core/compat/fake.rs",
            "TYPE",
            "Fake",
            "druid_rdbc::spi::Fake",
            "druid::core::fake",
            "P04_TASK_5_DRUID_INTERNALIZATION",
        )
        with self.assertRaisesRegex(VerificationError, "identity hash drift"):
            require_identity_counter_hash(
                Counter({identity: 1}),
                "0" * 64,
                "Task 6 compatibility additions",
            )

    def test_live_repository_verification_executes_task4_handoff(self) -> None:
        """A mutated Task 4 handoff must fail the live Task 5 verifier."""

        with mock.patch.object(
            verifier,
            "verify_task4_evidence_handoff",
            side_effect=VerificationError("mutated Task 4 handoff reached"),
        ):
            with self.assertRaisesRegex(
                VerificationError, "mutated Task 4 handoff reached"
            ):
                verify_repository(repository_root())

    def test_admission_composition_requires_all_local_factory_identities(self) -> None:
        source = " ".join(
            [
                "DruidPool::new_standard",
                "sqlx_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool",
                "toasty_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool",
                "rbdc_standard_factory_is_admitted_borrowed_and_executed_by_druid_pool",
            ]
        )
        with self.assertRaisesRegex(VerificationError, "admission composition"):
            require_admission_composition_source(source)

    def test_shared_execution_policy_requires_both_entrypoint_families(self) -> None:
        policy = " ".join(
            [
                "struct DruidExecutionFilterPolicy",
                "before_execute(context)",
                "raw(effective_sql.clone()).await",
                "after_execute(context",
            ]
        )
        with self.assertRaisesRegex(VerificationError, "do not share"):
            require_shared_execution_policy(
                policy,
                "DruidExecutionFilterPolicy::execute",
                "DruidExecutionFilterPolicy::execute",
            )

    def test_task4_handoff_rejects_each_regenerated_manifest_hash(self) -> None:
        root = repository_root()
        for constant in (
            "TASK4_COVERAGE_MANIFEST_SHA256",
            "TASK4_RUNTIME_MANIFEST_SHA256",
        ):
            with self.subTest(constant=constant), mock.patch.object(
                verifier, constant, "0" * 64
            ):
                with self.assertRaises(VerificationError):
                    verify_task4_evidence_handoff(root)

    def test_task4_exact_rbdc_command_failure_is_not_bypassed(self) -> None:
        with mock.patch.object(
            verifier,
            "execute_evidence",
            side_effect=verifier.WrapperVerificationError(
                "forced current RBDC coverage failure"
            ),
        ):
            with self.assertRaisesRegex(
                VerificationError, "current RBDC coverage failure"
            ):
                verifier.execute_task5_rbdc_compiler_coverage(repository_root())

    def test_task4_handoff_executes_current_rbdc_compiler_coverage_v2(self) -> None:
        with mock.patch.object(verifier, "run_cargo_gate"), mock.patch.object(
            verifier, "execute_evidence", create=True
        ) as execute:
            verifier.execute_task5_rbdc_compiler_coverage(repository_root())
        execute.assert_called_once()
        evidence, coverage = execute.call_args.args[1:]
        self.assertEqual(evidence.family, "RBDC")
        rbdc = [family for family in coverage if family.family == "RBDC"]
        self.assertEqual(len(rbdc), 1)
        self.assertEqual(len(rbdc[0].requirements), 15)
        transitioned = [
            requirement
            for requirement in rbdc[0].requirements
            if requirement.source_path.as_posix()
            == "crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs"
        ]
        self.assertEqual(len(transitioned), 1)
        self.assertEqual(
            transitioned[0].source_sha256,
            verifier.TASK5_RBDC_EVIDENCE_SOURCE_SHA256,
        )

    def test_exact_runtime_contracts_bind_every_c3_and_registry_identity(self) -> None:
        with mock.patch.object(verifier, "run_cargo_gate"), mock.patch.object(
            verifier, "run_exact_cargo_test", create=True
        ) as exact:
            verify_task5_runtime_contracts(repository_root())
        names = {call.args[2].test_id for call in exact.call_args_list}
        self.assertEqual(
            names,
            {
                "canonical_prepare_rewrite_cache_parameters_batch_events_and_timeout_match_raw",
                "canonical_prepared_setter_failure_and_clear_keep_filter_snapshot_truthful",
                "canonical_wall_prepare_and_metadata_rejection_prevent_raw_calls",
                "canonical_result_set_open_initializes_wall_tenant_hidden_mapping",
                "canonical_result_set_open_rejection_closes_unregistered_raw_once",
                "canonical_result_set_value_metadata_was_null_filters_modify_and_reject_once",
                "canonical_prepared_generic_execute_wraps_registered_result_with_typed_params",
                "canonical_callable_generic_execute_uses_same_typed_policy_and_wraps_result",
                "canonical_prepared_generic_fatal_error_runs_after_once_and_discards",
                "canonical_prepared_multi_result_open_failure_closes_every_raw_once",
                "canonical_multi_result_cleanup_failure_preserves_primary_and_retries",
                "canonical_callable_multi_result_open_failure_closes_every_raw_once",
                "canonical_statement_multi_result_open_failure_closes_every_raw_once",
                "canonical_statement_batch_rewrites_orders_clears_and_filters_once",
                "canonical_statement_batch_denial_and_partial_fatal_error_are_classified_once",
                "fatal_standard_create_prepare_call_and_metadata_errors_force_discard",
                "core::standard_resource_registry::tests::late_registration_after_close_is_rejected_and_closed",
                "core::standard_resource_registry::tests::explicit_close_unregisters_strong_entries_during_long_lease",
                "core::standard_resource_registry::tests::concurrent_close_all_waits_for_one_exact_drain",
                "core::standard_resource_registry::tests::aborted_close_all_caller_does_not_cancel_registry_owned_drain",
                "core::standard_resource_registry::tests::dropping_registry_cancels_blocked_coordinator_task",
                "core::standard_resource_registry::tests::pending_admission_cancellation_unblocks_close",
                "core::standard_resource_registry::tests::multiple_pending_admissions_all_finish_before_close_returns",
                "core::standard_resource_registry::tests::close_all_waits_for_rejected_late_resource_close_completion",
                "core::standard_resource_registry::tests::cancelled_rejected_registration_keeps_registry_cleanup_ownership",
                "core::standard_resource_registry::tests::late_registration_close_failure_stays_old_generation_and_blocks_reopen",
                "late_metadata_result_registration_is_closed_before_next_lease",
                "late_result_close_failure_discards_old_generation_without_reopen",
            },
        )

    def test_pending_admission_policy_rejects_permit_moved_after_raw_acquisition(
        self,
    ) -> None:
        root = repository_root()
        producer_sources = {
            path.name: path.read_text(encoding="utf-8")
            for path in (
                root / "crates/druid/src/core/druid_pooled_connection.rs",
                root / "crates/druid/src/core/druid_pooled_statement.rs",
                root / "crates/druid/src/core/druid_pooled_prepared_statement.rs",
                root / "crates/druid/src/core/druid_pooled_callable_statement.rs",
                root / "crates/druid/src/core/database_meta_data_proxy_impl.rs",
                root / "crates/druid/src/core/druid_pooled_result_set.rs",
            )
        }
        source = producer_sources["druid_pooled_connection.rs"]
        permit = (
            "        let admission = "
            "resources.begin_admission(policy.resource_generation())?;\n"
        )
        raw = "        let result = self.standard_mut()?.create_statement(options).await;\n"
        self.assertEqual(source.count(raw), 1)
        self.assertEqual(source.count(permit + raw), 1)
        producer_sources["druid_pooled_connection.rs"] = source.replace(
            permit + raw,
            raw + permit,
            1,
        )
        with self.assertRaisesRegex(VerificationError, "pre-raw admission"):
            verifier.require_pending_admission_policy(
                (
                    root / "crates/druid/src/core/standard_resource_registry.rs"
                ).read_text(encoding="utf-8"),
                (
                    root / "crates/druid/src/core/standard_resource_admission.rs"
                ).read_text(encoding="utf-8"),
                producer_sources,
            )

    def test_residual_method_delegation_rejects_direct_raw_fallback(self) -> None:
        source = """
            impl druid_rdbc::statement::PreparedStatement for DruidPooledPreparedStatement {
                async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
                    self.raw.execute_prepared().await
                }
            }
        """
        with self.assertRaisesRegex(VerificationError, "shared policy delegation"):
            verifier.require_method_delegation(
                source,
                "druid_rdbc::statement::PreparedStatement",
                "DruidPooledPreparedStatement",
                {"execute_prepared": "execute_standard_prepared_results"},
                "druid_pooled_prepared_statement.rs",
            )

    def test_pending_admission_policy_rejects_missing_producer_permit(self) -> None:
        with self.assertRaisesRegex(VerificationError, "pending admission"):
            verifier.require_pending_admission_policy(
                "struct StandardResourceRegistry; fn begin_admission() {}",
                "struct StandardResourceAdmission;",
                {
                    "druid_pooled_connection.rs": "begin_admission begin_admission begin_admission",
                    "druid_pooled_statement.rs": "begin_standard_admission begin_standard_admission begin_standard_admission",
                    "druid_pooled_prepared_statement.rs": "begin_standard_admission begin_standard_admission begin_standard_admission",
                    "druid_pooled_callable_statement.rs": "begin_standard_admission",
                    "database_meta_data_proxy_impl.rs": "begin_admission",
                    "druid_pooled_result_set.rs": "register(admission)",
                },
            )

    def test_exact_runtime_source_rejects_name_in_comment_only(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "fake.rs"
            source.write_text(
                "// fn required_behavior() {}\n#[test]\nfn another_test() {}\n",
                encoding="utf-8",
            )
            spec = verifier.ExactCargoTestSpec(
                Path("fake.rs"), "lib", "required_behavior"
            )
            with self.assertRaisesRegex(VerificationError, "missing or ambiguous"):
                verifier.require_exact_test_source(root, spec)

    def test_exact_runtime_rejects_successful_zero_test_process(self) -> None:
        spec = verifier.ExactCargoTestSpec(
            Path("crates/druid/tests/standard_filter_stat_semantics_test.rs"),
            "test:standard_filter_stat_semantics_test",
            "canonical_prepare_rewrite_cache_parameters_batch_events_and_timeout_match_raw",
        )
        zero = verifier.subprocess.CompletedProcess(
            [],
            0,
            b"running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured\n",
            b"",
        )
        with mock.patch.object(
            verifier, "require_exact_test_source"
        ), mock.patch.object(verifier, "run_cargo_gate", return_value=zero):
            with self.assertRaisesRegex(
                VerificationError, "did not execute exactly one"
            ):
                verifier.run_exact_cargo_test(repository_root(), "mutated exact", spec)

    def test_live_runtime_contract_failure_is_propagated(self) -> None:
        with mock.patch.object(
            verifier,
            "run_cargo_gate",
            side_effect=VerificationError("mutated Task 5 runtime reached"),
        ):
            with self.assertRaisesRegex(
                VerificationError, "mutated Task 5 runtime reached"
            ):
                verify_task5_runtime_contracts(repository_root())


if __name__ == "__main__":
    unittest.main()
