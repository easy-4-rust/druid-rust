import com.alibaba.druid.filter.Filter;
import com.alibaba.druid.filter.FilterAdapter;
import com.alibaba.druid.filter.FilterManager;
import java.lang.reflect.Method;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
public final class FilterClassIdentityOracle {
    public static void main(String[] args) throws Exception {
        Filter adapter = new FilterAdapter() {};
        String name = adapter.getClass().getName();
        Method exists = FilterManager.class.getDeclaredMethod("existsFilter", List.class, String.class);
        exists.setAccessible(true);
        List<Filter> filters = Collections.singletonList(adapter);
        String[] candidates = {name, name.toUpperCase(Locale.ROOT), name.toLowerCase(Locale.ROOT), name + " "};
        String[] labels = {"exact", "uppercase", "lowercase", "trailing_space"};
        for (int i = 0; i < candidates.length; i++) {
            System.out.println(labels[i] + "\t" + exists.invoke(null, filters, candidates[i]));
        }
    }
}
