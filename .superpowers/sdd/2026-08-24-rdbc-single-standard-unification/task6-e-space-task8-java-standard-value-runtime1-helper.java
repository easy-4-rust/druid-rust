import java.sql.JDBCType;
import java.sql.Types;
import java.sql.SQLType;
import java.sql.PseudoColumnUsage;
import java.sql.RowId;
import java.io.InputStream;
import java.security.MessageDigest;

public final class StandardValueOracle {
    private static String classHash(Class<?> type) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = type.getResourceAsStream("/" + type.getName().replace('.', '/') + ".class")) {
            if (input == null) throw new AssertionError("missing class resource " + type.getName());
            byte[] buffer = new byte[4096];
            int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        StringBuilder result = new StringBuilder();
        for (byte value : digest.digest()) result.append(String.format("%02x", value & 255));
        return result.toString();
    }
    public static void main(String[] args) throws Exception {
        Class<?>[] types = { JDBCType.class, Types.class, SQLType.class, PseudoColumnUsage.class, RowId.class };
        for (Class<?> type : types) {
            if (type.getClassLoader() != null) throw new AssertionError("not bootstrap-loaded " + type.getName());
            System.out.println("CLASS\t" + type.getName() + "\t" + classHash(type));
        }
        if (JDBCType.values().length != 39) throw new AssertionError("JDBCType count");
        for (JDBCType value : JDBCType.values()) {
            int number = value.getVendorTypeNumber();
            if (!"java.sql".equals(value.getVendor())) throw new AssertionError("vendor");
            if (!value.name().equals(value.getName())) throw new AssertionError("name");
            if (JDBCType.valueOf(number) != value) throw new AssertionError("lookup");
            if (Types.class.getField(value.getName()).getInt(null) != number) throw new AssertionError("Types constant");
            System.out.println("TYPE\t" + value.getName() + "\t" + value.getVendor() + "\t" + number);
        }
        int[] unknown = {Integer.MIN_VALUE, Integer.MAX_VALUE, 9999, -9999};
        for (int number : unknown) {
            try {
                JDBCType.valueOf(number);
                throw new AssertionError("accepted unknown " + number);
            } catch (IllegalArgumentException expected) {
                System.out.println("UNKNOWN\t" + number + "\t" + expected.getClass().getName());
            }
        }
        String[] expectedStates = {"SELECT_LIST_ONLY", "WHERE_CLAUSE_ONLY", "NO_USAGE_RESTRICTIONS", "USAGE_UNKNOWN"};
        if (PseudoColumnUsage.values().length != expectedStates.length) throw new AssertionError("pseudo count");
        for (int i = 0; i < expectedStates.length; i++) {
            if (!PseudoColumnUsage.values()[i].name().equals(expectedStates[i])) throw new AssertionError("pseudo identity");
            System.out.println("PSEUDO\t" + expectedStates[i]);
        }
        System.out.println("PASS\t39 types, 39 lookups, 4 unknown rejections, 4 pseudo states");
    }
}
