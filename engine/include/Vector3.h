#pragma once

#include "Core.h"

namespace BE::Math
{
	/// @todo Use SIMD register.
	class BS_API Vector3 {
	public:
		static const Vector3 Zero;
		static const Vector3 One;
		static const Vector3 UnitX;
		static const Vector3 UnitY;
		static const Vector3 UnitZ;

		float x;
		float y;
		float z;

		constexpr Vector3() noexcept;
		constexpr Vector3(float inX, float inY, float inZ) noexcept;
		Vector3(float* elems) noexcept;

		void Set(float inX, float inY, float inZ) noexcept;

		float& operator[](uint8 index) noexcept;
		const float& operator[](uint8 index) const noexcept;

		Vector3 operator+() const noexcept;
		Vector3 operator-() const noexcept;

		Vector3& operator+=(const Vector3& other) noexcept;
		Vector3& operator-=(const Vector3& other) noexcept;
		Vector3& operator*=(const Vector3& other) noexcept;
		Vector3& operator*=(const float scalar) noexcept;
		Vector3& operator/=(const Vector3& other) noexcept;
		Vector3& operator/=(const float scalar) noexcept;

		float LengthSquared() const noexcept;
		float Length() const noexcept;

		void Normalized() noexcept;
		static Vector3 Normalize(const Vector3& vec) noexcept;

		static float Dot(const Vector3& lhs, const Vector3& rhs) noexcept;
		static Vector3 Cross(const Vector3& lhs, const Vector3& rhs) noexcept;
		static Vector3 Reflect(const Vector3& v, const Vector3& n) noexcept;

		static Vector3 Transform(const Vector3& v, const class Quaternion& q) noexcept;
		static Vector3 Transform(const Vector3& vec, const class Matrix4x4& mat, float w = 1.0f) noexcept;
		static Vector3 TransformWithPerspDiv(const Vector3& vec, const class Matrix4x4& mat, float w = 1.0f) noexcept;

		// freinds
		friend Vector3 operator+(const Vector3& lhs, const Vector3& rhs) noexcept;
		friend Vector3 operator-(const Vector3& lhs, const Vector3& rhs) noexcept;
		friend Vector3 operator*(const Vector3& lhs, const Vector3& rhs) noexcept;
		friend Vector3 operator*(const Vector3& vec, const float scalar) noexcept;
		friend Vector3 operator*(const float scalar, const Vector3& vec) noexcept;
		friend Vector3 operator/(const Vector3& lhs, const Vector3& rhs) noexcept;
		friend Vector3 operator/(const Vector3& vec, const float scalar) noexcept;

		/// @warning Do not use it as an operator for the underlying API.
		explicit operator const float* () const noexcept
		{
			return &x;
		}
	};

	inline constexpr Vector3::Vector3() noexcept
		: x(0.0f), y(0.0f), z(0.0f) {}

	inline constexpr Vector3::Vector3(float inX, float inY, float inZ) noexcept
		: x(inX), y(inY), z(inZ) {}

	inline Vector3::Vector3(float* elems) noexcept
		: x(elems[0]), y(elems[1]), z(elems[2]) {}

	inline void Vector3::Set(float inX, float inY, float inZ) noexcept
	{
		x = inX;
		y = inY;
		z = inZ;
	}

	inline float& Vector3::operator[](uint8 index) noexcept
	{
		check(index < 4);
		return (&x)[index];
	}

	inline const float& Vector3::operator[](uint8 index) const noexcept
	{
		check(index < 4);
		return (&x)[index];
	}

	inline Vector3 Vector3::operator+() const noexcept
	{
		return *this;
	}

	inline Vector3 Vector3::operator-() const noexcept
	{
		return *this * -1.0f;
	}

	inline Vector3& Vector3::operator+=(const Vector3& other) noexcept
	{
		x += other.x;
		y += other.y;
		z += other.z;
		return *this;
	}

	inline Vector3& Vector3::operator-=(const Vector3& other) noexcept
	{
		x -= other.x;
		y -= other.y;
		z -= other.z;
		return *this;
	}

	inline Vector3& Vector3::operator*=(const Vector3& other) noexcept
	{
		x *= other.x;
		y *= other.y;
		z *= other.z;
		return *this;
	}

	inline Vector3& Vector3::operator*=(const float scalar) noexcept
	{
		x *= scalar;
		y *= scalar;
		z *= scalar;
		return *this;
	}

	inline Vector3& Vector3::operator/=(const Vector3& other) noexcept
	{
		x /= other.x;
		y /= other.y;
		z /= other.z;
		return *this;
	}

	inline Vector3& Vector3::operator/=(const float scalar) noexcept
	{
		x /= scalar;
		y /= scalar;
		z += scalar;
		return *this;
	}

	inline void Vector3::Normalized() noexcept
	{
		*this /= Length();
	}

	inline Vector3 Vector3::Normalize(const Vector3& vec) noexcept
	{
		return vec / vec.Length();
	}

	inline float Vector3::Dot(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return (lhs.x * rhs.x) + (lhs.y * rhs.y) + (lhs.z * rhs.z);
	}

	inline Vector3 Vector3::Cross(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{
			lhs.y * rhs.z - lhs.z * rhs.y,
			lhs.z * rhs.x - lhs.x * rhs.z,
			lhs.x * rhs.y - lhs.y * rhs.x
		};
	}

	inline Vector3 Vector3::Reflect(const Vector3& v, const Vector3& n) noexcept
	{
		return v - 2.0f * Vector3::Dot(v, n) * n;
	}

	inline Vector3 operator+(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs.x + rhs.x, lhs.y + rhs.y, lhs.z + rhs.z };
	}

	inline Vector3 operator-(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z };
	}

	inline Vector3 operator*(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs.x * rhs.x, lhs.y * rhs.y, lhs.z * rhs.z };
	}

	inline Vector3 operator*(const Vector3& vec, const float scalar) noexcept
	{
		return Vector3{ vec.x * scalar, vec.y * scalar, vec.z * scalar };
	}

	inline Vector3 operator*(const float scalar, const Vector3& vec) noexcept
	{
		return Vector3{ vec.x * scalar, vec.y * scalar, vec.z * scalar };
	}

	inline Vector3 operator/(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs.x / rhs.x, lhs.y / rhs.y, lhs.z / rhs.z };
	}

	inline Vector3 operator/(const Vector3& vec, const float scalar) noexcept
	{
		return Vector3{ vec.x / scalar, vec.y / scalar, vec.z / scalar };
	}
}