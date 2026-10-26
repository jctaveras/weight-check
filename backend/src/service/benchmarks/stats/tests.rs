use super::*;

#[test]
fn summary_of_known_values() {
    let s = summarize(&[4.0, 1.0, 3.0, 2.0, 5.0]).unwrap();
    assert_eq!(s.count, 5);
    assert_eq!(s.mean, 3.0);
    assert_eq!(s.median, 3.0);
    assert_eq!(s.min, 1.0);
    assert_eq!(s.max, 5.0);
    assert!((s.p95 - 4.8).abs() < 1e-9);
    assert!((s.stddev - 2f64.sqrt()).abs() < 1e-9);
}

#[test]
fn empty_and_non_finite() {
    assert!(summarize(&[]).is_none());
    assert!(summarize(&[f64::NAN]).is_none());
    assert_eq!(summarize(&[7.0]).unwrap().p95, 7.0);
}

#[test]
fn deltas_between_values() {
    assert_eq!(deltas(&[10.0, 15.0, 25.0]), vec![5.0, 10.0]);
    assert!(deltas(&[1.0]).is_empty());
}
