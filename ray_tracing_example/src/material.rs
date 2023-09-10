use crate::color::Color;
use crate::hittable::HitRecord;
use crate::ray::Ray;
use crate::{common, vec3};

pub trait Material: Send + Sync {
    fn scatter(
        &self,
        r_in: &Ray,
        rec: &HitRecord,
        attenuation: &mut Color,
        scattered: &mut Ray,
    ) -> bool;
}

pub struct Lambertian {
    albedo: Color,
}

impl Lambertian {
    pub fn new(a: Color) -> Lambertian {
        Lambertian { albedo: a }
    }
}

impl Material for Lambertian {
    fn scatter(
        &self,
        _r_in: &Ray,
        rec: &HitRecord,
        attenuation: &mut Color,
        scattered: &mut Ray,
    ) -> bool {
        let mut scatter_direction = rec.normal + vec3::random_unit_vector();

        // Catch degenerate scatter direction
        if scatter_direction.near_zero() {
            scatter_direction = rec.normal;
        }

        *attenuation = self.albedo;
        *scattered = Ray::new(rec.p, scatter_direction);
        true
    }
}

pub struct Metal {
    albedo: Color,
    fuzz: f64,
}

impl Metal {
    pub fn new(a: Color, f: f64) -> Metal {
        Metal {
            albedo: a,
            fuzz: if f < 1.0 { f } else { 1.0 },
        }
    }
    // pub fn new(a: Color) -> Metal {
    //     Metal { albedo: a }
    // }
}

impl Material for Metal {
    fn scatter(
        &self,
        r_in: &Ray,
        rec: &HitRecord,
        attenuation: &mut Color,
        scattered: &mut Ray,
    ) -> bool {
        let reflected = vec3::reflect(vec3::unit_vector(r_in.direction()), rec.normal);

        *attenuation = self.albedo;
        // *scattered = Ray::new(rec.p, reflected);
        *scattered = Ray::new(rec.p, reflected + self.fuzz * vec3::random_in_unit_sphere());
        vec3::dot(scattered.direction(), rec.normal) > 0.0
    }
}

pub struct Dielectric {
    ir: f64, // Index of refraction
}

impl Dielectric {
    pub fn new(index_of_refraction: f64) -> Dielectric {
        Dielectric {
            ir: index_of_refraction,
        }
    }

    fn reflectance(cosine: f64, ref_idx: f64) -> f64 {
        // Use Schlick's approximation for reflectance
        let mut r0 = (1.0 - ref_idx) / (1.0 + ref_idx);
        r0 = r0 * r0;
        r0 + (1.0 - r0) * f64::powf(1.0 - cosine, 5.0)
    }
}

impl Material for Dielectric {
    fn scatter(
        &self,
        r_in: &Ray,
        rec: &HitRecord,
        attenuation: &mut Color,
        scattered: &mut Ray,
    ) -> bool {
        let refraction_ratio = if rec.front_face {
            1.0 / self.ir
        } else {
            self.ir
        };

        let unit_direction = vec3::unit_vector(r_in.direction());
        let cos_theta = f64::min(vec3::dot(-unit_direction, rec.normal), 1.0);
        let sin_theta = f64::sqrt(1.0 - cos_theta * cos_theta);

        let cannot_refract = refraction_ratio * sin_theta > 1.0;
        let direction = if cannot_refract
            || Self::reflectance(cos_theta, refraction_ratio) > common::random_double()
        {
            vec3::reflect(unit_direction, rec.normal)
        } else {
            vec3::refract(unit_direction, rec.normal, refraction_ratio)
        };

        *attenuation = Color::new(1.0, 1.0, 1.0);
        *scattered = Ray::new(rec.p, direction);
        true
    }
}
