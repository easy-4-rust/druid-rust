import com.alibaba.druid.DbType;
import com.alibaba.druid.sql.SQLUtils;
import com.alibaba.druid.sql.ast.SQLStatement;
import com.alibaba.druid.sql.visitor.ParameterizedOutputVisitorUtils;
import com.alibaba.druid.util.JdbcConstants;

import java.util.List;

/**
 * P2 golden oracle: run the same 32 dialect cases through Druid 1.2.28 and emit TSV.
 *
 * Usage: java -cp druid-1.2.28.jar:. DialectGolden > java-golden.tsv
 */
public final class DialectGolden {
    private static final class Case {
        private final DbType dbType;
        private final String id;
        private final String sql;

        Case(DbType dbType, String id, String sql) {
            this.dbType = dbType;
            this.id = id;
            this.sql = sql;
        }

        DbType dbType() { return dbType; }
        String id() { return id; }
        String sql() { return sql; }
    }

    private static String oneLine(String value) {
        return value.replace("\t", " ").replace("\n", " ").replace("\r", " ");
    }

    private static String parameterize(String sql, DbType dbType) {
        try {
            return ParameterizedOutputVisitorUtils.parameterize(sql, dbType);
        } catch (Throwable error) {
            return "ERR:" + error.getClass().getSimpleName() + ":" + error.getMessage();
        }
    }

    public static void main(String[] args) {
        Case[] cases = new Case[] {
            new Case(DbType.oracle, "oracle-date-literal", "SELECT * FROM emp WHERE hiredate > DATE '2020-01-01'"),
            new Case(DbType.oracle, "oracle-rownum", "SELECT * FROM t WHERE ROWNUM <= 10"),
            new Case(DbType.oracle, "oracle-connect-by", "SELECT LEVEL, SYS_CONNECT_BY_PATH(name, '/') FROM t START WITH id = 1 CONNECT BY PRIOR id = parent_id"),
            new Case(DbType.oracle, "oracle-merge", "MERGE INTO t USING s ON (t.id = s.id) WHEN MATCHED THEN UPDATE SET t.v = s.v WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v)"),
            new Case(DbType.oracle, "oracle-returning-into", "INSERT INTO t (id) VALUES (1) RETURNING id INTO :out_id"),
            new Case(DbType.oracle, "oracle-table-function", "SELECT * FROM TABLE(fn(1))"),
            new Case(DbType.oracle, "oracle-decode-nvl", "SELECT NVL(a, 0), DECODE(b, 1, 'x', 'y') FROM dual"),
            new Case(DbType.oracle, "oracle-identity", "CREATE TABLE t (id NUMBER(10) GENERATED ALWAYS AS IDENTITY, v VARCHAR2(20))"),
            new Case(DbType.odps, "odps-lateral-view", "SELECT * FROM t LATERAL VIEW explode(arr) v AS x"),
            new Case(DbType.odps, "odps-distribute-sort", "SELECT * FROM t DISTRIBUTE BY k SORT BY v"),
            new Case(DbType.odps, "odps-insert-overwrite-partition", "INSERT OVERWRITE TABLE t PARTITION (ds='2020') SELECT * FROM s"),
            new Case(DbType.odps, "odps-create-partitioned", "CREATE TABLE t (a STRING) PARTITIONED BY (ds STRING)"),
            new Case(DbType.odps, "odps-mapjoin-hint", "SELECT /*+ MAPJOIN(b) */ * FROM a JOIN b ON a.id = b.id"),
            new Case(DbType.odps, "odps-trans-array", "SELECT TRANS_ARRAY(1, ',', arr, ',') FROM t"),
            new Case(DbType.odps, "odps-limit", "SELECT * FROM t LIMIT 10"),
            new Case(DbType.odps, "odps-cast", "SELECT CAST(a AS BIGINT) FROM t"),
            new Case(DbType.hive, "hive-lateral-view", "SELECT * FROM t LATERAL VIEW explode(arr) v AS x"),
            new Case(DbType.hive, "hive-insert-partition", "INSERT OVERWRITE TABLE t PARTITION (ds='2020') SELECT * FROM s"),
            new Case(DbType.hive, "hive-mapjoin", "SELECT /*+ MAPJOIN(b) */ a.* FROM a JOIN b ON a.id = b.id"),
            new Case(DbType.hive, "hive-stored-parquet", "CREATE TABLE t (a STRING) STORED AS PARQUET"),
            new Case(DbType.hive, "hive-cluster-by", "SELECT * FROM t CLUSTER BY k"),
            new Case(DbType.hive, "hive-tablesample", "SELECT * FROM t TABLESAMPLE (10 PERCENT)"),
            new Case(DbType.hive, "hive-named-struct", "SELECT named_struct('a', 1) FROM t"),
            new Case(DbType.hive, "hive-partition-filter", "SELECT * FROM t WHERE ds = '2020'"),
            new Case(DbType.doris, "doris-select", "SELECT * FROM t WHERE k1 = 1"),
            new Case(DbType.doris, "doris-create-unique-key", "CREATE TABLE t (k1 INT, v INT) UNIQUE KEY(k1) DISTRIBUTED BY HASH(k1) BUCKETS 3 PROPERTIES (\"replication_num\" = \"1\")"),
            new Case(DbType.doris, "doris-distribute-buckets", "SELECT * FROM t DISTRIBUTE BY HASH(k1) BUCKETS 10"),
            new Case(DbType.doris, "doris-in-subquery", "SELECT * FROM t WHERE k1 IN (SELECT k1 FROM s)"),
            new Case(DbType.doris, "doris-insert-select", "INSERT INTO t SELECT * FROM s"),
            new Case(DbType.doris, "doris-bitmap-agg", "SELECT bitmap_union_count(b) FROM t GROUP BY k"),
            new Case(DbType.doris, "doris-order-limit", "SELECT * FROM t ORDER BY k1 LIMIT 10"),
            new Case(DbType.doris, "doris-alter-add-column", "ALTER TABLE t ADD COLUMN c INT"),
        };

        System.out.println("dialect\tcase\tparse\tformat\treparse\tparameterized\tsql");
        for (Case item : cases) {
            String parse;
            String format;
            String reparse = "n/a";
            try {
                List<SQLStatement> statements = SQLUtils.parseStatements(item.sql(), item.dbType());
                parse = "ok";
                format = oneLine(SQLUtils.toSQLString(statements, item.dbType()));
                try {
                    SQLUtils.parseStatements(format, item.dbType());
                    reparse = "ok";
                } catch (Throwable error) {
                    reparse = "err";
                }
            } catch (Throwable error) {
                parse = "err";
                format = "ERR:" + error.getClass().getSimpleName() + ":" + error.getMessage();
            }
            String parameterized = oneLine(parameterize(item.sql(), item.dbType()));
            System.out.println(item.dbType().name() + "\t" + item.id() + "\t" + parse + "\t" + oneLine(format) + "\t" + reparse + "\t" + parameterized + "\t" + oneLine(item.sql()));
        }
    }
}
