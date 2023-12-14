use cargo_export::target_file_name;
use proptest::prelude::*;

#[derive(Debug)]
struct Tag(Option<String>);

#[derive(Debug)]
struct Ext(Option<String>);

prop_compose! {
    fn any_tag()(tag_name in any::<Option<String>>()) -> Tag {
        Tag(tag_name)
    }
}

prop_compose! {
    fn any_ext()(ext in "(|exe|dylib|so|dll)") -> Ext {
        Ext(Some(ext).filter(|e| !e.is_empty()))
    }
}

proptest! {
    #[test]
    fn doesnt_crash(name in "[a-z]+", tag in any_tag(), ext in any_ext(), hash in any::<Option<u64>>()) {
        let Tag(tag) = tag;
        let Ext(ext) = ext;
        let mut input = name.clone();
        if let Some(hash) = &hash {
            input.push('-');
            input.push_str(&format!("{:016x}", hash));
        }
        if let Some(ext) = &ext {
            input.push('.');
            input.push_str(ext);
        }
        let result = target_file_name(&input, tag.as_deref());

        assert!(result.starts_with(&name));
        let mut expected_min_length = name.len();

        if let Some(tag) = tag {
            assert!(result.contains(&tag));
            expected_min_length += tag.len();
        }
        if let Some(ext) = &ext {
            assert!(result.ends_with(&format!(".{}", ext)));
            expected_min_length += ext.len();
        }
        if let Some(hash) = hash {
            assert_eq!(None, result.find(&format!("{:016x}", hash)));
        }

        assert!(result.len() >= expected_min_length);
    }
}
