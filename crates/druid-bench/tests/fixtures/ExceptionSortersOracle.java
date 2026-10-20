import com.alibaba.druid.pool.ExceptionSorter;
import com.alibaba.druid.pool.vendor.DB2ExceptionSorter;
import com.alibaba.druid.pool.vendor.InformixExceptionSorter;
import com.alibaba.druid.pool.vendor.MySqlExceptionSorter;
import com.alibaba.druid.pool.vendor.NullExceptionSorter;
import com.alibaba.druid.pool.vendor.OceanBaseOracleExceptionSorter;
import com.alibaba.druid.pool.vendor.OracleExceptionSorter;
import com.alibaba.druid.pool.vendor.PGExceptionSorter;
import com.alibaba.druid.pool.vendor.PhoenixExceptionSorter;
import com.alibaba.druid.pool.vendor.SybaseExceptionSorter;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.sql.SQLException;
import java.sql.SQLRecoverableException;
import java.util.HashSet;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.Properties;

/** 对共享异常语料逐行执行冻结的 Druid 分类器，输出实际结果。 */
public final class ExceptionSortersOracle {
    private ExceptionSortersOracle() {
    }

    public static void main(String[] args) throws Exception {
        List<String> results = new ArrayList<String>();
        for (String line : Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8)) {
            if (line.startsWith("#") || line.isEmpty()) {
                continue;
            }
            String[] fields = line.split("\t", -1);
            if (fields.length != 8) {
                throw new IllegalArgumentException("invalid case: " + fields[0]);
            }
            ExceptionSorter sorter = sorter(fields[1]);
            if (!"-".equals(fields[7])) {
                Properties properties = new Properties();
                properties.setProperty("druid.oracle.fatalErrorCodes", fields[7]);
                sorter.configFromProperties(properties);
            }
            String state = "-".equals(fields[3]) ? null : fields[3];
            String message = "-".equals(fields[4]) ? null : fields[4];
            int code = Integer.parseInt(fields[2]);
            SQLException exception = "true".equals(fields[5])
                    ? new SQLRecoverableException(message, state, code)
                    : new SQLException(message, state, code);
            Throwable previous = exception;
            if (!"-".equals(fields[6])) {
                for (String kind : fields[6].split(",")) {
                    Throwable cause;
                    if ("socket".equals(kind)) {
                        cause = new SocketTimeoutException();
                    } else if ("ordinary".equals(kind)) {
                        cause = new Exception();
                    } else {
                        throw new IllegalArgumentException("unknown cause: " + kind);
                    }
                    previous.initCause(cause);
                    previous = cause;
                }
            }
            try {
                results.add(fields[0] + "\t" + sorter.isExceptionFatal(exception));
            } catch (RuntimeException failure) {
                results.add(fields[0] + "\tthrows:" + failure.getClass().getName());
            }
        }
        // 分类器自己的日志保留在 stdout/stderr，结果文件使用固定 UTF-8 编码。
        Files.write(Paths.get(args[1]), results, StandardCharsets.UTF_8);
    }

    private static ExceptionSorter sorter(String name) {
        switch (name) {
            case "mysql": return new MySqlExceptionSorter();
            case "pg": return new PGExceptionSorter();
            case "phoenix": return new PhoenixExceptionSorter();
            case "db2": return new DB2ExceptionSorter();
            case "informix": return new InformixExceptionSorter();
            case "sybase": return new SybaseExceptionSorter();
            case "null": return NullExceptionSorter.getInstance();
            case "oracle": {
                OracleExceptionSorter sorter = new OracleExceptionSorter();
                sorter.setFatalErrorCodes(new HashSet<Integer>());
                return sorter;
            }
            case "oceanbase_oracle": {
                OceanBaseOracleExceptionSorter sorter = new OceanBaseOracleExceptionSorter();
                sorter.setFatalErrorCodes(new HashSet<Integer>());
                return sorter;
            }
            default: throw new IllegalArgumentException("unknown sorter: " + Objects.requireNonNull(name));
        }
    }
}
