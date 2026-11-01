import java.util.*;
import com.alibaba.druid.pool.vendor.OracleExceptionSorter;
public class SorterPropertyProbe {
 public static void main(String[] args) {
  for (String input : new String[]{"１２３４５","١٢٣٤٥","+१२३४५","-٢١٤٧٤٨٣٦٤٨","𝟙𝟚","²","2147483648"," 1"}) {
   OracleExceptionSorter sorter = new OracleExceptionSorter();
   sorter.setFatalErrorCodes(new TreeSet<Integer>());
   Properties p = new Properties(); p.setProperty("druid.oracle.fatalErrorCodes", input);
   sorter.configFromProperties(p);
   System.out.println(input + "\t" + sorter.getFatalErrorCodes());
  }
  for(int c=0;c<=65535;c++) if(Character.digit((char)c,10)==0) System.out.printf("ZERO U+%04X%n",c);
 }
}
