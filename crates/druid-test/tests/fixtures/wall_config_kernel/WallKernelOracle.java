import com.alibaba.druid.wall.WallConfig;
import java.lang.reflect.*;
import java.util.*;
public class WallKernelOracle {
  static void emit(String key, Object value) { System.out.println(key+"\t"+value); }
  public static void main(String[] args) throws Exception {
    WallConfig config = new WallConfig();
    TreeMap<String,String> defaults = new TreeMap<>();
    for(Field f: WallConfig.class.getDeclaredFields()) {
      if(Modifier.isStatic(f.getModifiers())) continue;
      if(f.getType()==boolean.class || f.getType()==int.class) {
        f.setAccessible(true); defaults.put(f.getName(), String.valueOf(f.get(config)));
      }
    }
    for(Map.Entry<String,String> row: defaults.entrySet()) emit("default."+row.getKey(),row.getValue());
    String[] sets={"denyFunctions","denyTables","denySchemas","denyVariants","denyObjects","permitFunctions","permitTables","permitSchemas","permitVariants","readOnlyTables"};
    for(String dialect:new String[]{"clickhouse","mysql","oracle","postgres","sqlserver"}) {
      WallConfig rules=new WallConfig("META-INF/druid/wall/"+dialect);
      for(String name:sets) {
        Field f=WallConfig.class.getDeclaredField(name); f.setAccessible(true);
        Collection<?> values=(Collection<?>)f.get(rules);
        TreeSet<String> sorted=new TreeSet<>();
        for(Object value:values) sorted.add((String)value);
        emit(dialect+"."+name,String.join(",",sorted));
      }
    }
    Class<?> suite=Class.forName("com.alibaba.druid.bvt.filter.wall.WallConfigTest");
    int count=0;
    for(Method method:suite.getDeclaredMethods()) {
      if(method.getName().startsWith("test_")) {
        method.invoke(suite.getConstructor().newInstance()); count++;
      }
    }
    if(count!=8) throw new AssertionError("source test denominator changed: "+count);
    System.err.println("Original WallConfigTest methods passed: "+count);
  }
}
