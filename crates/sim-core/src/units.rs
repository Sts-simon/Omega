use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum UnitError {
    NonFinite(&'static str),
    Negative(&'static str),
    NotPositive(&'static str),
}

impl fmt::Display for UnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite(name) => write!(formatter, "{name} must be finite"),
            Self::Negative(name) => write!(formatter, "{name} cannot be negative"),
            Self::NotPositive(name) => write!(formatter, "{name} must be greater than zero"),
        }
    }
}

impl std::error::Error for UnitError {}

macro_rules! si_non_negative {
    ($type_name:ident, $constructor:ident, $getter:ident, $label:literal) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
        pub struct $type_name(f64);

        impl $type_name {
            pub fn $constructor(value: f64) -> Result<Self, UnitError> {
                if !value.is_finite() {
                    return Err(UnitError::NonFinite($label));
                }
                if value < 0.0 {
                    return Err(UnitError::Negative($label));
                }
                Ok(Self(value))
            }

            pub fn $getter(self) -> f64 {
                self.0
            }
        }
    };
}

macro_rules! si_positive {
    ($type_name:ident, $constructor:ident, $getter:ident, $label:literal) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
        pub struct $type_name(f64);

        impl $type_name {
            pub fn $constructor(value: f64) -> Result<Self, UnitError> {
                if !value.is_finite() {
                    return Err(UnitError::NonFinite($label));
                }
                if value <= 0.0 {
                    return Err(UnitError::NotPositive($label));
                }
                Ok(Self(value))
            }

            pub fn $getter(self) -> f64 {
                self.0
            }
        }
    };
}

si_positive!(Length, meters, as_meters, "Length");
si_non_negative!(Mass, kilograms, as_kilograms, "Mass");
si_positive!(Temperature, kelvin, as_kelvin, "Absolute temperature");
si_non_negative!(Duration, seconds, as_seconds, "Duration");
si_positive!(Density, kilograms_per_cubic_meter, as_kg_per_m3, "Density");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_keeps_si_value() {
        let length = Length::meters(1.25).unwrap();
        assert!((length.as_meters() - 1.25).abs() < f64::EPSILON);
    }

    #[test]
    fn negative_mass_is_rejected() {
        assert_eq!(Mass::kilograms(-0.1), Err(UnitError::Negative("Mass")));
    }

    #[test]
    fn zero_length_is_rejected() {
        assert_eq!(Length::meters(0.0), Err(UnitError::NotPositive("Length")));
    }

    #[test]
    fn non_finite_temperature_is_rejected() {
        assert_eq!(
            Temperature::kelvin(f64::NAN),
            Err(UnitError::NonFinite("Absolute temperature"))
        );
    }
}
