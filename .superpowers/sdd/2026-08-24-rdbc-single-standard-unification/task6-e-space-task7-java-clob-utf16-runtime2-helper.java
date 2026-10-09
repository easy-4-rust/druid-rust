import java.sql.Clob;
import java.sql.NClob;
import java.sql.SQLException;
import javax.sql.rowset.serial.SerialClob;
import java.io.InputStream;
import java.security.MessageDigest;
public class ClobUtf16Oracle2 {
 static String hex(byte[] bytes) {StringBuilder out=new StringBuilder();for(byte b:bytes)out.append(String.format("%02x",b&255));return out.toString();}
 static void classHash(Class<?> c)throws Exception {MessageDigest md=MessageDigest.getInstance("SHA-256");InputStream in=c.getResourceAsStream("/"+c.getName().replace('.','/')+".class");if(in==null)throw new IllegalStateException(c.getName());byte[] buf=new byte[4096];int n;while((n=in.read(buf))!=-1)md.update(buf,0,n);in.close();System.out.println("CLASS\t"+c.getName()+"\t"+hex(md.digest()));}
 static void value(String key,Object value){System.out.println("VALUE\t"+key+"\t"+value);}
 static String units(String value){StringBuilder out=new StringBuilder();for(int i=0;i<value.length();i++){if(i>0)out.append(',');out.append(String.format("%04x",(int)value.charAt(i)));}return out.toString();}
 public static void main(String[] args)throws Exception {
  System.out.println("HOST\tjava.version\t"+System.getProperty("java.version"));classHash(Clob.class);classHash(NClob.class);classHash(SerialClob.class);
  Clob c=new SerialClob("A\uD83D\uDE00Z".toCharArray());
  value("length",c.length());value("high_surrogate",units(c.getSubString(2,1)));
  value("position_emoji",c.position("\uD83D\uDE00",1));value("position_z",c.position("Z",1));
  try {value("write_low_surrogate",c.setString(1,"\uD83D\uDE00",1,1));} catch(Exception failure) {value("write_low_surrogate","EXCEPTION:"+failure.getClass().getName());}value("after_write",units(c.getSubString(1,4)));
  c.truncate(2);value("after_truncate",units(c.getSubString(1,2)));
  c.free();c.free();boolean closed=false;try{c.length();}catch(SQLException ex){closed=true;}value("closed_sql_exception",closed);
 }
}
