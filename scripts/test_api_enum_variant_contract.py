"""API 枚举成员识别合同；历史输出只作为格式夹具，不能认证当前产品。"""
import unittest
from verify_jdbc42_final_runtime import verify_api_shapes
from verify_jdbc42_semantic_inventory import VerificationError
class EnumVariantContract(unittest.TestCase):
 def check(self,text,target='druid_rdbc::error::SqlExceptionKind::Data',shape='ENUM_KIND'):
  verify_api_shapes([['java:java.sql.SQLDataException#TYPE','java.sql.SQLDataException','SQLDataException',shape,'MATCH',target]],text)
 def test_real_unit_variant_is_accepted(self):
  self.check('pub enum druid_rdbc::error::SqlExceptionKind\npub druid_rdbc::error::SqlExceptionKind::Data\n')
 def test_variant_before_enum_is_accepted(self):
  self.check('pub druid_rdbc::error::SqlExceptionKind::Data\npub enum druid_rdbc::error::SqlExceptionKind\n')
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
