use twig_cli::creds;

#[test]
fn test_creds_module_access() {
  let netrc_path = creds::get_netrc_path();
  assert!(netrc_path.ends_with(".netrc"));
}
