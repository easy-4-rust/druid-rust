from pathlib import Path
zeroes=[int(line.split('+')[1],16) for line in Path('D:/codex-build/druid-sorter-methods-20261101/logs/probe-java.log').read_text(encoding='utf-8', errors='replace').splitlines() if line.startswith('ZERO ')]
assert len(zeroes)==37
boundaries=[('max',2147483647,'+２１４７４８３６４７'),('min',-2147483648,'-٢١٤٧٤٨٣٦٤٨'),('overflow',2147483647,'2147483648'),('underflow',-2147483648,'-2147483649'),('space_before',12345,' 12345'),('space_after',12345,'12345 '),('nbsp',12345,'\u00a012345'),('sign_only',0,'+'),('double_sign',12345,'++12345'),('minus_plus',12345,'-+12345'),('wide_plus',12345,'＋12345'),('unicode_minus',12345,'−12345'),('supplementary',12345,'𝟙𝟚𝟛𝟜𝟝'),('superscript',2,'²'),('fraction',1,'½'),('empty',0,''),('mixed',12345,'1٢３४5'),('zero_prefix',12345,'０００１２３４５'),('invalid_then_valid',12345,'a,１２３４５'),('duplicates',12345,',１２３４５,,12345,'),('negative_zero',0,'-٠'),('negative_code',-12345,'-١٢٣٤٥')]
rows=[]
for vendor in ('oracle','oceanbase_oracle'):
 for zero in zeroes:
  value=''.join(chr(zero+int(c)) for c in '1234567890')
  rows.append(f'property_{vendor}_digits_{zero:04x}\t{vendor}\t1234567890\t-\tordinary\tfalse\t-\t{value}')
 for name,code,prop in boundaries:
  rows.append(f'property_{vendor}_{name}\t{vendor}\t{code}\t-\tordinary\tfalse\t-\t{prop}')
p=Path('crates/druid-bench/tests/fixtures/exception_sorter_property_cases.tsv')
p.write_text(f'# cases={len(rows)}\n'+'\n'.join(rows)+'\n',encoding='utf-8')
print(len(rows))

