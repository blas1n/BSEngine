#pragma once

#include "Core.h"
#include <Eigen/Dense>
#include <utility>

namespace BE::Math
{
	class Vector3;
	class Vector4;

	class BS_API Vector2 final
	{
	public:
		Vector2() noexcept : vec{ } {}
		
		Vector2(const Vector2& other) noexcept : vec{ other.vec } {}
		Vector2(Vector2&& other) noexcept : vec{ std::move(other.vec) } {}

		explicit Vector2(const float x, const float y) noexcept
			: vec{ x, y } {}

		explicit Vector2(const float elems[2]) noexcept
			: vec{ elems } {}

		~Vector2() = default;

		inline void Set(const float x, const float y) noexcept
		{
			vec << x, y;
		}

		inline float& x() noexcept { return (*this)[0]; }
		inline float x() const noexcept { return (*this)[0]; }

		inline float& y() noexcept { return (*this)[1]; }
		inline float y() const noexcept { return (*this)[1]; }

		inline Vector2& operator=(const Vector2& other) noexcept
		{
			vec = other.vec;
			return *this;
		}

		inline Vector2& operator=(Vector2&& other) noexcept
		{
			vec = std::move(other.vec);
			return *this;
		}

		inline float& operator[](const Uint8 index) noexcept
		{
			return vec[index];
		}

		inline float operator[](const Uint8 index) const noexcept
		{
			return vec[index];
		}

		inline Vector2 operator-() const noexcept;

		inline bool operator==(const Vector2& other) const noexcept
		{
			return vec == other.vec;
		}

		inline bool operator!=(const Vector2& other) const noexcept
		{
			return vec != other.vec;
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

		inline float* Data() noexcept { return vec.data(); }
		inline const float* Data() const noexcept { return vec.data(); }

		static inline float Dot(const Vector2& lhs, const Vector2& rhs) noexcept
		{
			return lhs.vec.dot(rhs.vec);
		}

		static inline Vector2 Reflect(const Vector2& v, const Vector2& n) noexcept;

		static inline Vector2 Zero() noexcept
		{
			Vector2 ret;
			ret.vec = Eigen::Vector2f::UnitX();
			return ret;
		}

		static inline Vector2 One() noexcept
		{
			Vector2 ret;
			ret.vec = Eigen::Vector2f::Ones();
			return ret;
		}

		static inline Vector2 UnitX() noexcept
		{
			Vector2 ret;
			ret.vec = Eigen::Vector2f::UnitX();
			return ret;
		}

		static inline Vector2 UnitY() noexcept
		{
			Vector2 ret;
			ret.vec = Eigen::Vector2f::UnitY();
			return ret;
		}

		explicit operator Vector3() const noexcept;
		explicit operator Vector4() const noexcept;

	private:
		Eigen::Vector2f vec;
	};

	inline Vector2 operator+(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs } += rhs;
	}

	inline Vector2 operator-(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs } -= rhs;
	}

	inline Vector2 operator*(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs } *= rhs;
	}

	inline Vector2 operator*(const Vector2& vec, const float scalar) noexcept
	{
		return Vector2{ vec } *= scalar;
	}

	inline Vector2 operator*(const float scalar, const Vector2& vec) noexcept
	{
		return Vector2{ vec } *= scalar;
	}

	inline Vector2 operator/(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs } /= rhs;
	}

	inline Vector2 operator/(const Vector2& vec, const float scalar) noexcept
	{
		return Vector2{ vec } /= scalar;
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