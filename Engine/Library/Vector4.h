#pragma once

#include "Core.h"
#include <Eigen/Dense>

namespace BE::Math
{
	class BS_API Vector4
	{
	public:
		Vector4() noexcept : vec{ } {}

		explicit Vector4(const float inX, const float inY) noexcept
			: vec{ inX, inY } {}

		explicit Vector4(const float elems[4]) noexcept
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

		inline Vector4& operator+=(const Vector4& other) noexcept
		{
			vec += other.vec;
			return *this;
		}

		inline Vector4& operator-=(const Vector4& other) noexcept
		{
			vec -= other.vec;
			return *this;
		}

		inline Vector4& operator*=(const Vector4& other) noexcept
		{
			vec[0] *= other.vec[0];
			vec[1] *= other.vec[1];
			vec[2] *= other.vec[2];
			vec[3] *= other.vec[3];
			return *this;
		}

		inline Vector4& operator*=(const float scalar) noexcept
		{
			vec *= scalar;
			return *this;
		}

		inline Vector4& operator/=(const Vector4& other) noexcept
		{
			vec[0] /= other.vec[0];
			vec[1] /= other.vec[1];
			vec[2] /= other.vec[2];
			vec[3] /= other.vec[3];
			return *this;
		}

		inline Vector4& operator/=(const float scalar) noexcept
		{
			vec /= scalar;
			return *this;
		}

		inline Vector4 operator+() const noexcept
		{
			return *this;
		}

		inline Vector4 operator-() const noexcept
		{
			return *this * -1.0f;
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

		inline Vector4 Normalized() const noexcept
		{
			Vector4 ret;
			ret.vec = vec.normalized();
			return ret;
		}

		static inline float Dot(const Vector4& lhs, const Vector4& rhs) noexcept
		{
			return lhs.vec.dot(rhs.vec);
		}

		static inline Vector4 Cross3(const Vector4& lhs, const Vector4& rhs) noexcept
		{
			Vector4 ret;
			ret.vec = lhs.vec.cross3(rhs.vec);
			return ret;
		}

		static inline Vector4 Reflect(const Vector4& v, const Vector4& n) noexcept
		{
			return v - 2.0f * Vector4::Dot(v, n) * n;
		}

	private:
		Eigen::Vector4f vec;
	};

	inline Vector4 operator+(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Vector4 operator-(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Vector4 operator*(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Vector4 operator*(const Vector4& vec, const float scalar) noexcept
	{
		auto ret = vec;
		return ret *= scalar;
	}

	inline Vector4 operator*(const float scalar, const Vector4& vec) noexcept
	{
		auto ret = vec;
		return ret *= scalar;
	}

	inline Vector4 operator/(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		auto ret = lhs;
		return ret /= rhs;
	}

	inline Vector4 operator/(const Vector4& vec, const float scalar) noexcept
	{
		auto ret = vec;
		return ret /= scalar;
	}
}