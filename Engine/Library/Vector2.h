#pragma once

#include "Core.h"
#include <Eigen/Dense>

namespace BE::Math
{
	class BS_API Vector2
	{
	public:
		Vector2() noexcept : vec{ } {}
		
		explicit Vector2(const float inX, const float inY) noexcept
			: vec{ inX, inY } {}

		explicit Vector2(const float elems[2]) noexcept
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

		inline Vector2 operator+() const noexcept
		{
			return *this;
		}

		inline Vector2 operator-() const noexcept
		{
			return *this * -1.0f;
		}

		inline Vector2& operator+=(const Vector2& other) noexcept
		{
			vec += other.vec;
			return *this;
		}

		inline Vector2& operator-=(const Vector2& other) noexcept
		{
			vec -= other.vec;
			return *this;
		}

		inline Vector2& operator*=(const Vector2& other) noexcept
		{
			vec[0] *= other.vec[0];
			vec[1] *= other.vec[1];
			return *this;
		}

		inline Vector2& operator*=(const float scalar) noexcept
		{
			vec *= scalar;
			return *this;
		}

		inline Vector2& operator/=(const Vector2& other) noexcept
		{
			vec[0] /= other.vec[0];
			vec[1] /= other.vec[1];
			return *this;
		}

		inline Vector2& operator/=(const float scalar) noexcept
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

		inline Vector2 Normalized() const noexcept
		{
			Vector2 ret;
			ret.vec = vec.normalized();
			return ret;
		}

		static inline float Dot(const Vector2& lhs, const Vector2& rhs) noexcept
		{
			return lhs.vec.dot(rhs.vec);
		}

		static inline Vector2 Reflect(const Vector2& v, const Vector2& n) noexcept
		{
			return v - 2.0f * Vector2::Dot(v, n) * n;
		}

	private:
		Eigen::Vector2f vec;
	};

	Vector2 operator+(Vector2 lhs, const Vector2& rhs) noexcept
	{
		return lhs += rhs;
	}

	Vector2 operator-(Vector2 lhs, const Vector2& rhs) noexcept
	{
		return lhs += rhs;
	}

	Vector2 operator*(Vector2 lhs, const Vector2& rhs) noexcept
	{
		return lhs += rhs;
	}

	Vector2 operator*(Vector2 vec, const float scalar) noexcept
	{
		return vec *= scalar;
	}

	Vector2 operator*(const float scalar, Vector2 vec) noexcept
	{
		return vec *= scalar;
	}

	Vector2 operator/(Vector2 lhs, const Vector2& rhs) noexcept
	{
		return lhs /= rhs;
	}

	Vector2 operator/(Vector2 vec, const float scalar) noexcept
	{
		return vec /= scalar;
	}
}