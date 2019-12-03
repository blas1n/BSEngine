#pragma once

#include "Core.h"
#include <Eigen/Dense>

namespace BE::Math
{
	class BS_API Vector3
	{
	public:
		Vector3() noexcept
			: vec{ } {}

		explicit Vector3(const float inX, const float inY) noexcept
			: vec{ inX, inY } {}

		explicit Vector3(const float elems[3]) noexcept
			: vec{ elems } {}

		inline void Set(const float inX, const float inY) noexcept
		{
			vec << inX, inY;
		}

		inline float& operator[](const Uint8 index) noexcept
		{
			return vec[index];
		}

		inline const float& operator[](const Uint8 index) const noexcept
		{
			return vec[index];
		}

		inline Vector3 operator+() const noexcept
		{
			return *this;
		}

		inline Vector3 operator-() const noexcept
		{
			return *this * -1.0f;
		}

		inline Vector3& operator+=(const Vector3& other) noexcept
		{
			vec += other.vec;
			return *this;
		}

		inline Vector3& operator-=(const Vector3& other) noexcept
		{
			vec -= other.vec;
			return *this;
		}

		inline Vector3& operator*=(const Vector3& other) noexcept
		{
			vec[0] *= other.vec[0];
			vec[1] *= other.vec[1];
			return *this;
		}

		inline Vector3& operator*=(const float scalar) noexcept
		{
			vec *= scalar;
			return *this;
		}

		inline Vector3& operator/=(const Vector3& other) noexcept
		{
			vec[0] /= other.vec[0];
			vec[1] /= other.vec[1];
			return *this;
		}

		inline Vector3& operator/=(const float scalar) noexcept
		{
			vec /= scalar;
			return *this;
		}

		inline float LengthSquared() const noexcept
		{
			vec.squaredNorm();
		}

		inline float Length() const noexcept
		{
			vec.norm();
		}

		inline void Normalize() noexcept
		{
			vec.normalize();
		}

		inline Vector3 Normalized() const noexcept
		{
			Vector3 ret;
			ret.vec = vec.normalized();
			return ret;
		}

		static inline float Dot(const Vector3& lhs, const Vector3& rhs) noexcept
		{
			return lhs.vec.dot(rhs.vec);
		}

		static inline Vector3 Cross(const Vector3& lhs, const Vector3& rhs) noexcept
		{
			Vector3 ret;
			ret.vec = lhs.vec.cross(rhs.vec);
			return ret;
		}

		static inline Vector3 Reflect(const Vector3& v, const Vector3& n) noexcept
		{
			return v - 2.0f * Vector3::Dot(v, n) * n;
		}

	private:
		Eigen::Vector3f vec;
	};

	Vector3 operator+(Vector3 lhs, const Vector3& rhs) noexcept
	{
		return lhs += rhs;
	}

	Vector3 operator-(Vector3 lhs, const Vector3& rhs) noexcept
	{
		return lhs += rhs;
	}

	Vector3 operator*(Vector3 lhs, const Vector3& rhs) noexcept
	{
		return lhs += rhs;
	}

	Vector3 operator*(Vector3 vec, const float scalar) noexcept
	{
		return vec *= scalar;
	}

	Vector3 operator*(const float scalar, Vector3 vec) noexcept
	{
		return vec *= scalar;
	}

	Vector3 operator/(Vector3 lhs, const Vector3& rhs) noexcept
	{
		return lhs /= rhs;
	}

	Vector3 operator/(Vector3 vec, const float scalar) noexcept
	{
		return vec /= scalar;
	}
}