#[derive(Debug, Clone, Copy)]
pub struct SpringDamper {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SpringDamper {
    pub fn new(initial: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
            stiffness: 260.0,
            damping: 26.0,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn step(&mut self, dt: f32) -> bool {
        let clamped_dt = dt.clamp(0.001, 0.050);
        let displacement = self.value - self.target;
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = spring_force + damping_force;

        self.velocity += acceleration * clamped_dt;
        self.value += self.velocity * clamped_dt;

        if (self.value - self.target).abs() < 0.001 && self.velocity.abs() < 0.001 {
            self.value = self.target;
            self.velocity = 0.0;
            false
        } else {
            true
        }
    }
}
