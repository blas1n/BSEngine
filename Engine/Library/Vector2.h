#pragma once

#include "Core.h"
#include <Eigen/Dense>

namespace BE::Math
{
	class BS_API Vector2 final
	{
	public:
		Vector2() noexcept : vec{ } {}
		
		explicit Vector2(const float x, const float y) noexcept
			: vec{ x, y } {}

		explicit Vector2(const float elems[2]) noexcept
			: vec{ elems } {}

		inline void Set(const float x, const float y) noexcept
		{
			vec << x, y;
		}

		inline float& operator[](const Uint8 index) noexcept
		{
			return vec[index];
		}

		inline const float& operator[](const Uint8 index) const noexcept
		{
			return vec[index];
		}

		inline Vector2 operator-() const noexcept;

		static inline bool operator==(const Vector2& lhs, const Vector2& rhs)
		{
			return lhs.vec == rhs.vec;
		}

		static inline bool operator!=(const Vector2& lhs, const Vector2& rhs)
		{
			return lhs.vec != rhs.vec;
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
			return vec.squaredNorm();
		}

		inline float Length() const noexcept
		{
			return vec.norm();
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

		static inline Vector2 Reflect(const Vector2& v, const Vector2& n) noexcept;

	private:
		Eigen::Vector2f vec;
	};

	inline Vector2 operator+(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Vector2 operator-(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Vector2 operator*(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Vector2 operator*(const Vector2& vec, const float scalar) noexcept
	{
		auto ret = vec;
		return ret *= scalar;
	}

	inline Vector2 operator*(const float scalar, const Vector2& vec) noexcept
	{
		auto ret = vec;
		return ret *= scalar;
	}

	inline Vector2 operator/(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		auto ret = lhs;
		return ret /= rhs;
	}

	inline Vector2 operator/(const Vector2& vec, const float scalar) noexcept
	{
		auto ret = vec;
		return ret /= scalar;
	}

	inline Vector2 Vector2::operator-() const noexcept
	{
		return *this * -1.0f;
	}

	inline Vector2 Vector2::Reflect(const Vector2& v, const Vector2& n) noexcept
	{
		return v - 2.0f * Vector2::Dot(v, n) * n;
	}
}