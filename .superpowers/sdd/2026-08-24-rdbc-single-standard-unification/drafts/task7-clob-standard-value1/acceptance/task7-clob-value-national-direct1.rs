#[tokio::test]
async fn target_admission_precedence_covers_both_verified_identity_orders(){
    // UUID身份随机；排序三个同能力句柄，明确让关闭模式处于两个目标之间。
    let mut values=(0..3).map(|_|{let p=probe(ResourceOperations::READ|ResourceOperations::FREE);(clob(p.clone()),p)}).collect::<Vec<_>>();
    values.sort_by(|a,b|a.0.resource_id().cmp(b.0.resource_id()));let (high,high_probe)=values.pop().unwrap();let (pattern,_)=values.pop().unwrap();let (low,low_probe)=values.pop().unwrap();
    assert!(low.resource_id()<pattern.resource_id());assert!(high.resource_id()>pattern.resource_id());pattern.free().await.unwrap();
    for(value,p)in [(low,low_probe),(high,high_probe)]{assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert!(p.calls.lock().unwrap().is_empty());value.free().await.unwrap();assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLException");}
}
#[tokio::test]
async fn national_public_methods_delegate_all_inherited_operations(){
    let p=probe(ResourceOperations::clob());let n=n_clob(p.clone());let view=n.as_clob().clone();
    assert_eq!(n.length().await.unwrap(),4);assert_eq!(n.get_sub_string(2,1).await.unwrap().as_utf16(),&[0xD83D]);
    assert_eq!(n.position_string(&RdbcString::from("😀"),1).await.unwrap(),Some(2));assert_eq!(n.position_clob(&view,1).await.unwrap(),Some(1));
    let full=n.get_character_stream().await.unwrap();assert_eq!(Some(full.clone()),*p.reader.lock().unwrap());assert_eq!(full.read_to_end_utf16().unwrap(),vec![65,0xD83D,0xDE00,90]);
    let ranged=n.get_character_stream_range(3,1).await.unwrap();assert_eq!(Some(ranged.clone()),*p.reader.lock().unwrap());assert_eq!(ranged.read_to_end_utf16().unwrap(),vec![0xDE00]);
    let ascii=n.get_ascii_stream().await.unwrap();assert_eq!(Some(ascii.clone()),*p.input.lock().unwrap());assert_eq!(ascii.read_to_end().unwrap(),b"A??Z");
    assert_eq!(n.set_string(1,&RdbcString::from("文")).await.unwrap(),1);assert_eq!(n.set_string_range(2,&RdbcString::from("😀"),1,1).await.unwrap(),1);
    let writer=n.set_character_stream(3).await.unwrap();assert_eq!(Some(writer.clone()),*p.writer.lock().unwrap());writer.write_utf16(&[0xDFFF]).unwrap();writer.close().unwrap();
    let output=n.set_ascii_stream(4).await.unwrap();assert_eq!(Some(output.clone()),*p.output.lock().unwrap());output.write(b"Q").unwrap();output.close().unwrap();
    assert_eq!(view.get_sub_string(1,4).await.unwrap().as_utf16(),&[0x6587,0xDE00,0xDFFF,81]);n.truncate(3).await.unwrap();assert_eq!(view.length().await.unwrap(),3);
    assert_eq!(&p.calls.lock().unwrap()[..12],&["length","get_sub_string","position_string","position_clob","get_character_stream","get_character_stream_range","get_ascii_stream","set_string","set_string_range","set_character_stream","set_ascii_stream","get_sub_string"]);
    assert_eq!(n.capabilities(),view.capabilities());assert_eq!(n.resource_id(),view.resource_id());n.free().await.unwrap();view.free().await.unwrap();assert!(n.is_freed());assert_eq!(p.frees.load(Ordering::SeqCst),1);
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let pooled_n=ResourceHandleFactory::n_clob(connection.create_n_clob().await.unwrap());let pooled_c=ResourceHandleFactory::clob(connection.create_clob().await.unwrap());
    assert_eq!(pooled_n.position_clob(&pooled_c,1).await.unwrap(),Some(1));assert_eq!(pooled_c.position_clob(pooled_n.as_clob(),1).await.unwrap(),Some(1));connection.close().await.unwrap();pooled_n.free().await.unwrap();pooled_c.free().await.unwrap();
    assert!(pooled_n.is_freed());assert!(pooled_c.is_freed());for probe in control.probes.lock().unwrap().iter(){assert_eq!(probe.calls.load(Ordering::SeqCst),1);}assert_eq!(control.returns.lock().unwrap().len(),1);
}
