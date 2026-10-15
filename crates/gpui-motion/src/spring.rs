use gpui::{
    AnimationExt, ElementId, IntoElement, SpringAnimation, SpringAnimationElement, SpringConfig,
    Styled,
};

/// Reusable spring parameters for GPUI element animation.
#[derive(Clone, Copy, Debug)]
pub struct SpringSpec {
    /// GPUI's spring configuration.
    pub config: SpringConfig,
    /// Distance from the target at which the spring is considered settled.
    pub epsilon: f32,
}

/// Adds a scalar spring animation to any styled GPUI element.
pub trait AnimateSpring: Styled + IntoElement + Sized + 'static {
    /// Animates from `from` to `to` and applies each sampled value to `self`.
    fn animate_spring(
        self,
        id: impl Into<ElementId>,
        from: f32,
        to: f32,
        spring: SpringSpec,
        apply: impl Fn(Self, f32) -> Self + 'static,
    ) -> SpringAnimationElement<Self> {
        self.with_spring(
            id,
            SpringAnimation::new(spring.config)
                .with_epsilon(spring.epsilon)
                .to(to)
                .from(from),
            apply,
        )
    }
}

impl<E: Styled + IntoElement + 'static> AnimateSpring for E {}
