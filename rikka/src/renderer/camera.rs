use std::{f32::consts::FRAC_PI_2, time::Duration};

use winit::{dpi::PhysicalPosition, event::*};

use rikka_core::{
    glm,
    nalgebra::{Matrix4, Vector3},
};

const SAFE_FRAC_PI_2: f32 = FRAC_PI_2 - 0.0001;
const UP_VECTOR: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

pub struct View {
    position: Vector3<f32>,
    yaw: f32,
    pitch: f32,
    matrix: Matrix4<f32>,
}

impl View {
    pub fn new(position: Vector3<f32>, yaw: f32, pitch: f32) -> Self {
        let mut view = Self {
            position,
            yaw,
            pitch,
            matrix: Matrix4::identity(),
        };
        view.calculate_matrix();
        view
    }

    pub fn matrix(&self) -> &Matrix4<f32> {
        &self.matrix
    }

    pub fn position(&self) -> &Vector3<f32> {
        &self.position
    }

    fn calculate_matrix(&mut self) {
        self.matrix = Matrix4::look_at_rh(
            &self.position.into(),
            &(self.position + self.forward()).into(),
            &UP_VECTOR,
        );

        // self.matrix = glm::look_at_lh(
        //     &self.position.into(),
        //     &(self.position + self.forward()).into(),
        //     &UP_VECTOR,
        // );
    }

    fn forward(&self) -> Vector3<f32> {
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        Vector3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize()
    }

    fn right(&self) -> Vector3<f32> {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize()
    }

    fn rotate_x(&mut self, amount: f32) {
        self.yaw += amount;
    }

    fn rotate_y(&mut self, amount: f32) {
        self.pitch += amount;
        if self.pitch < -SAFE_FRAC_PI_2 {
            self.pitch = -SAFE_FRAC_PI_2;
        } else if self.pitch > SAFE_FRAC_PI_2 {
            self.pitch = SAFE_FRAC_PI_2;
        }
    }
}

pub struct Projection {
    aspect: f32,
    width: u32,
    height: u32,

    fovy: f32,
    znear: f32,
    zfar: f32,
    matrix: Matrix4<f32>,
}

impl Projection {
    pub fn new(width: u32, height: u32, fovy: f32, znear: f32, zfar: f32) -> Self {
        let mut proj = Self {
            aspect: width as f32 / height as f32,
            width,
            height,
            fovy,
            znear,
            zfar,
            matrix: Matrix4::identity(),
        };
        proj.calculate_matrix();
        proj
    }

    pub fn matrix(&self) -> &Matrix4<f32> {
        &self.matrix
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
        self.width = width;
        self.height = height
    }

    fn calculate_matrix(&mut self) {
        // self.matrix = Matrix4::new_perspective(self.aspect, self.fovy, self.znear, self.zfar);

        // XXX: Fix perspective/view
        self.matrix = glm::perspective_rh_zo(self.aspect, self.fovy, self.znear, self.zfar);
        let v = self.matrix[(1, 1)];
        self.matrix[(1, 1)] = -v;
    }
}

pub struct FirstPersonCameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,

    rotate_horizontal: f32,
    rotate_vertical: f32,

    scroll: f32,
    speed: f32,
    sensitivity: f32,

    mouse_pressed: bool,
}

impl FirstPersonCameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,

            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,

            scroll: 0.0,
            speed,
            sensitivity,

            mouse_pressed: false,
        }
    }

    pub fn process_keyboard(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        let amount = if state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };

        match key {
            VirtualKeyCode::W | VirtualKeyCode::Up => {
                self.amount_forward = amount;
                true
            }
            VirtualKeyCode::S | VirtualKeyCode::Down => {
                self.amount_backward = amount;
                true
            }
            VirtualKeyCode::A | VirtualKeyCode::Left => {
                self.amount_left = amount;
                true
            }
            VirtualKeyCode::D | VirtualKeyCode::Right => {
                self.amount_right = amount;
                true
            }
            VirtualKeyCode::Space => {
                self.amount_up = amount;
                true
            }
            VirtualKeyCode::LShift => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }

    pub fn process_mouse_motion(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn process_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = -match delta {
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => *scroll as f32,
        }
    }

    pub fn update_view(&mut self, view: &mut View, dt: Duration) {
        let dt = dt.as_secs_f32();

        let forward = view.forward();
        let right = view.right();

        view.position += forward * (self.amount_forward - self.amount_backward) * self.speed * dt;
        view.position += right * (self.amount_right - self.amount_left) * self.speed * dt;

        view.position += -forward * self.scroll * self.speed * self.sensitivity * dt;
        self.scroll = 0.0;

        view.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        if self.mouse_pressed {
            view.rotate_x(self.rotate_horizontal * self.sensitivity * dt);
            view.rotate_y(-self.rotate_vertical * self.sensitivity * dt);
        }
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        // XXX: Only recalculate when something has changed
        view.calculate_matrix();
    }
}
