"""API 枚举成员识别合同；历史输出只作为格式夹具，不能认证当前产品。"""
import unittest
import json
from pathlib import Path
from verify_jdbc42_final_runtime import verify_api_shapes
from verify_jdbc42_semantic_inventory import (
 VerificationError, parse_semantic_payload, normalized_phase_digest,
 TASK4_ADAPTER_LIVE_ENTRIES, TASK5_LOGICAL_LIVE_ENTRIES,
)
class EnumVariantContract(unittest.TestCase):
 def check(self,text,target='druid_rdbc::error::SqlExceptionKind::Data',shape='ENUM_KIND'):
  verify_api_shapes([['java:java.sql.SQLDataException#TYPE','java.sql.SQLDataException','SQLDataException',shape,'MATCH',target]],text)
 def test_real_unit_variant_is_accepted(self):
  self.check('pub enum druid_rdbc::error::SqlExceptionKind\npub druid_rdbc::error::SqlExceptionKind::Data\n')
 def test_variant_before_enum_is_accepted(self):
  self.check('pub druid_rdbc::error::SqlExceptionKind::Data\npub enum druid_rdbc::error::SqlExceptionKind\n')
 def test_non_exhaustive_enum_and_its_variant_are_accepted(self):
  self.check('#[non_exhaustive] pub enum druid_rdbc::metadata::RowIdLifetime\npub druid_rdbc::metadata::RowIdLifetime::Forever\n',target='druid_rdbc::metadata::RowIdLifetime::Forever')
 def test_current_jdbc_inventory_matches_public_declaration_kinds(self):
  root=Path(__file__).resolve().parents[1]
  rows=parse_semantic_payload((root/'docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv').read_bytes())
  public_api=(root/'api/druid-rdbc.single-standard.after.txt').read_text(encoding='utf-8')
  verify_api_shapes(rows,public_api)
 def test_current_live_inventory_normalizes_to_frozen_oracle(self):
  root=Path(__file__).resolve().parents[1]
  rows=parse_semantic_payload((root/'docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv').read_bytes())
  oracle=json.loads((root/'docs/verification/rdbc-first/jdbc42-oracle.json').read_text(encoding='utf-8'))
  digest=normalized_phase_digest(rows,'logical-live',TASK4_ADAPTER_LIVE_ENTRIES,TASK5_LOGICAL_LIVE_ENTRIES)
  self.assertEqual(oracle['semantic_sha256'],digest)
  tampered=[row.copy() for row in rows]
  next(row for row in tampered if row[1]=='java.sql.DriverManager')[3]='MODULE'
  with self.assertRaises(VerificationError):
   normalized_phase_digest(tampered,'logical-live',TASK4_ADAPTER_LIVE_ENTRIES,TASK5_LOGICAL_LIVE_ENTRIES)
 def test_member_without_declared_enum_is_rejected(self):
  with self.assertRaises(VerificationError):self.check('pub druid_rdbc::error::SqlExceptionKind::Data\n')
 def test_missing_variant_is_not_inferred_from_parent(self):
  with self.assertRaises(VerificationError):self.check('pub enum druid_rdbc::error::SqlExceptionKind\n')
 def test_associated_constant_cannot_impersonate_enum_variant(self):
  with self.assertRaises(VerificationError):self.check('pub struct druid_rdbc::error::SqlExceptionKind\npub const druid_rdbc::error::SqlExceptionKind::Data: druid_rdbc::error::SqlExceptionKind\n')
 def test_enum_variant_does_not_satisfy_dto(self):
  with self.assertRaises(VerificationError):self.check('pub enum druid_rdbc::error::SqlExceptionKind\npub druid_rdbc::error::SqlExceptionKind::Data\n',shape='DTO')
 def test_variant_field_does_not_impersonate_variant(self):
  with self.assertRaises(VerificationError):self.check('pub enum druid_rdbc::error::SqlExceptionKind\npub druid_rdbc::error::SqlExceptionKind::Data\npub druid_rdbc::error::SqlExceptionKind::Data::field\n',target='druid_rdbc::error::SqlExceptionKind::Data::field')
if __name__=='__main__':unittest.main()
