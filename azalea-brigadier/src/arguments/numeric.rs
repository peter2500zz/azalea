use std::ops::RangeInclusive;

pub(crate) fn validate_bounds<T: PartialOrd + Copy>(minimum: Option<T>, maximum: Option<T>) {
    for bound in [minimum, maximum].into_iter().flatten() {
        assert!(
            bound.partial_cmp(&bound).is_some(),
            "numeric bounds must not be NaN"
        );
    }
    if let (Some(minimum), Some(maximum)) = (minimum, maximum) {
        assert!(minimum <= maximum, "minimum must not exceed maximum");
    }
}

pub(crate) fn bounds<T: PartialOrd + Copy>(range: RangeInclusive<T>) -> (T, T) {
    assert!(
        !range.is_empty(),
        "numeric range must be non-empty and ordered"
    );
    range.into_inner()
}

macro_rules! impl_numeric_config {
    ($parser:ident, $number:ty) => {
        impl $parser {
            /// Set the inclusive lower bound, preserving any upper bound.
            ///
            /// # Panics
            /// If the bounds are inconsistent or contain NaN.
            pub fn min(mut self, minimum: $number) -> Self {
                $crate::arguments::numeric::validate_bounds(Some(minimum), self.maximum);
                self.minimum = Some(minimum);
                self
            }

            /// Set the inclusive upper bound, preserving any lower bound.
            ///
            /// # Panics
            /// If the bounds are inconsistent or contain NaN.
            pub fn max(mut self, maximum: $number) -> Self {
                $crate::arguments::numeric::validate_bounds(self.minimum, Some(maximum));
                self.maximum = Some(maximum);
                self
            }

            /// Replace both bounds with an inclusive range.
            ///
            /// # Panics
            /// If the range is empty, exhausted, reversed, or contains NaN.
            pub fn range(mut self, range: std::ops::RangeInclusive<$number>) -> Self {
                let (minimum, maximum) = $crate::arguments::numeric::bounds(range);
                self.minimum = Some(minimum);
                self.maximum = Some(maximum);
                self
            }
        }

        impl<S, R> $crate::builder::argument_builder::ArgumentBuilder<S, R, $parser> {
            /// Set the inclusive lower bound without losing node metadata.
            ///
            /// # Panics
            /// If the bounds are inconsistent or contain NaN.
            pub fn min(self, minimum: $number) -> Self {
                self.configure_parser(|parser| parser.min(minimum))
            }

            /// Set the inclusive upper bound without losing node metadata.
            ///
            /// # Panics
            /// If the bounds are inconsistent or contain NaN.
            pub fn max(self, maximum: $number) -> Self {
                self.configure_parser(|parser| parser.max(maximum))
            }

            /// Replace both bounds with an inclusive range.
            ///
            /// # Panics
            /// If the range is empty, exhausted, reversed, or contains NaN.
            pub fn range(self, range: std::ops::RangeInclusive<$number>) -> Self {
                self.configure_parser(|parser| parser.range(range))
            }
        }
    };
}

pub(crate) use impl_numeric_config;
