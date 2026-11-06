//! One directory per framework.
//!
//! A provider may know as much about its framework as it likes. What it may not do is let that
//! knowledge out through anything except the [`FrameworkProvider`](crate::FrameworkProvider) seam.

pub mod drupal;
pub mod laravel;
pub mod mockery;
pub mod pest;
pub mod phpunit;
pub mod symfony;
pub mod twig;
