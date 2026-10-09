import java.sql.*;
import java.lang.reflect.*;
import java.security.MessageDigest;
import java.io.InputStream;
import java.util.*;
public final class ConstantExceptionOracle {
    private static String classHash(Class<?> type) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = type.getResourceAsStream("/" + type.getName().replace('.', '/') + ".class")) {
            if (input == null) throw new AssertionError("missing class resource");
            byte[] buffer = new byte[4096]; int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        StringBuilder result = new StringBuilder();
        for (byte value : digest.digest()) result.append(String.format("%02x", value & 255));
        return result.toString();
    }
    private static String kind(SQLException e) {
        if (e instanceof SQLTransientConnectionException) return "TransientConnection";
        if (e instanceof SQLTimeoutException) return "Timeout";
        if (e instanceof SQLTransactionRollbackException) return "TransactionRollback";
        if (e instanceof SQLTransientException) return "Transient";
        if (e instanceof SQLDataException) return "Data";
        if (e instanceof SQLIntegrityConstraintViolationException) return "IntegrityConstraintViolation";
        if (e instanceof SQLInvalidAuthorizationSpecException) return "InvalidAuthorization";
        if (e instanceof SQLNonTransientConnectionException) return "NonTransientConnection";
        if (e instanceof SQLSyntaxErrorException) return "Syntax";
        if (e instanceof SQLFeatureNotSupportedException) return "FeatureNotSupported";
        if (e instanceof SQLNonTransientException) return "NonTransient";
        if (e instanceof SQLRecoverableException) return "Recoverable";
        return "General";
    }
    private static void provenance(Class<?> type) throws Exception {
        if (type.getClassLoader() != null) throw new AssertionError("not bootstrap " + type);
        System.out.println("CLASS\t" + type.getName() + "\t" + classHash(type));
    }
    public static void main(String[] names) throws Exception {
        provenance(Types.class); provenance(SQLException.class);
        List<Field> fields = new ArrayList<>();
        for (Field f : Types.class.getDeclaredFields()) {
            if (f.getType() == int.class && Modifier.isPublic(f.getModifiers()) && Modifier.isStatic(f.getModifiers())) fields.add(f);
        }
        fields.sort(Comparator.comparing(Field::getName));
        if (fields.size() != 39) throw new AssertionError("constants " + fields.size());
        for (Field f : fields) System.out.println("CONSTANT\t" + f.getName() + "\t" + f.getInt(null));
        if (names.length != 12) throw new AssertionError("classes " + names.length);
        for (String name : names) {
            Class<?> type = Class.forName(name, false, null); provenance(type);
            for (String state : new String[] {null, "42S02"}) {
                SQLException e = (SQLException)type.getConstructor(String.class, String.class, int.class).newInstance("original", state, 17);
                if (e.getErrorCode() != 17 || !Objects.equals(e.getSQLState(), state) || !Objects.equals(e.getMessage(), "original")) throw new AssertionError("payload");
                System.out.println("KIND\t" + name + "\t" + String.valueOf(e.getSQLState()) + "\t" + kind(e) + "\t" + e.getErrorCode() + "\t" + e.getMessage());
            }
        }
    }
}
