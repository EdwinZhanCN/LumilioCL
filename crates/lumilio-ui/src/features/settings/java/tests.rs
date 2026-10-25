use super::*;

#[test]
fn the_download_dialog_offers_the_sources_majors_with_21_first_chosen() {
    let spec = download_spec(JavaSource::Mojang, &[8, 17, 21, 25]);
    let FieldKind::Pick { options, selected } = &spec.fields[0].kind else {
        panic!("a dropdown");
    };
    assert_eq!(options, &["Java 8", "Java 17", "Java 21", "Java 25"]);
    assert_eq!(*selected, Some(2));
    assert_eq!(
        (spec.parse)(&["Java 25".to_owned()]),
        Ok(LiveIntent::DownloadJava {
            source: JavaSource::Mojang,
            major: 25
        })
    );
    assert!((spec.parse)(&[String::new()]).is_err());
}

#[test]
fn unread_offers_fall_back_to_what_each_source_can_have() {
    let mut view = SettingsView::default();
    assert_eq!(offered(&view, JavaSource::Mojang), MOJANG_FALLBACK);
    assert_eq!(offered(&view, JavaSource::OracleGraalVm), [21, 25]);
    view.java_offers = vec![(JavaSource::Mojang, vec![17, 21, 25])];
    assert_eq!(offered(&view, JavaSource::Mojang), [17, 21, 25]);
}
