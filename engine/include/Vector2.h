#pragma once

#include "Core.h"

namespace BE::Math
{
	/// @todo Use SIMD register.
	class BS_API Vector2 {
	public:
		static const Vector2 Zero;
		static const Vector2 One;
		static const Vector2 UnitX;
		static const Vector2 UnitY;

		float x;
		float y;

		constexpr Vector2() noexcept;
		constexpr Vector2(float inX, float inY) noexcept;
		Vector2(float* elems) noexcept;

		void Set(float inX, float inY) noexcept;

		float& operator[](Uint8 index) noexcept;
		const float& operator[](Uint8 index) const noexcept;

		Vector2 operator+() const noexcept;
		Vector2 operator-() const noexcept;

		Vector2& operator+=(const Vector2& other) noexcept;
		Vector2& operator-=(const Vector2& other) noexcept;
		Vector2& operator*=(const Vector2& other) noexcept;
		Vector2& operator*=(const float scalar) noexcept;
		Vector2& operator/=(const Vector2& other) noexcept;
		Vector2& operator/=(const float scalar) noexcept;

		float LengthSquared() const noexcept;
		float Length() const noexcept;

		void Normalized() noexcept;
		static Vector2 Normalize(const Vector2& vec) noexcept;

		static float Dot(const Vector2& lhs, const Vector2& rhs) noexcept;
		static Vector2 Reflect(const Vector2& v, const Vector2& n) noexcept;
		static Vector2 Transform(const Vector2& vec, const class Matrix3x3& mat, float w = 1.0f) noexcept;

		friend Vector2 operator+(const Vector2& lhs, const Vector2& rhs) noexcept;
		friend Vector2 operator-(const Vector2& lhs, const Vector2& rhs) noexcept;
		friend Vector2 operator*(const Vector2& lhs, const Vector2& rhs) noexcept;
		friend Vector2 operator*(const Vector2& vec, const float scalar) noexcept;
		friend Vector2 operator*(const float scalar, const Vector2& vec) noexcept;
		friend Vector2 operator/(const Vector2& lhs, const Vector2& rhs) noexcept;
		friend Vector2 operator/(const Vector2& vec, const float scalar) noexcept;

		/// @warning Do not use it as an operator for the underlying API.
		explicit operator const float* () const noexcept
		{
			return &x;
		}
	};

	inline constexpr Vector2::Vector2() noexcept
		: x(0.0f), y(0.0f) {}

	inline constexpr Vector2::Vector2(float inX, float inY) noexcept
		: x(inX), y(inY) {}

	inline Vector2::Vector2(float* elems) noexcept
		: x(elems[0]), y(elems[1]) {}

	inline void Vector2::Set(float inX, float inY) noexcept
	{
		x = inX;
		y = inY;
	}

	inline float& Vector2::operator[](Uint8 index) noexcept
	{
		check(index < 3);
		return (&x)[index];
	}

	inline const float& Vector2::operator[](Uint8 index) const noexcept
	{
		check(index < 3);
		return (&x)[index];
	}

	inline Vector2 Vector2::operator+() const noexcept
	{
		return *this;
	}

	inline Vector2 Vector2::operator-() const noexcept
	{
		return *this * -1.0f;
	}

	inline Vector2& Vector2::operator+=(const Vector2& other) noexcept
	{
		x += other.x;
		y += other.y;
		return *this;
	}

	inline Vector2& Vector2::operator-=(const Vector2& other) noexcept
	{
		x -= other.x;
		y -= other.y;
		return *this;
	}

	inline Vector2& Vector2::operator*=(const Vector2& other) noexcept
	{
		x *= other.x;
		y *= other.y;
		return *this;
	}

	inline Vector2& Vector2::operator*=(const float scalar) noexcept
	{
		x *= scalar;
		y *= scalar;
		return *this;
	}

	inline Vector2& Vector2::operator/=(const Vector2& other) noexcept
	{
		x /= other.x;
		y /= other.y;
		return *this;
	}

	inline Vector2& Vector2::operator/=(const float scalar) noexcept
	{
		x /= scalar;
		y /= scalar;
		return *this;
	}

	inline void Vector2::Normalized() noexcept
	{
		*this /= Length();
	}

	inline Vector2 Vector2::Normalize(const Vector2& vec) noexcept
	{
		return vec / vec.Length();
	}

	inline float Vector2::Dot(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return (lhs.x * rhs.x) + (lhs.y * rhs.y);
	}

	inline Vector2 Vector2::Reflect(const Vector2& v, const Vector2& n) noexcept
	{
		return v - 2.0f * Vector2::Dot(v, n) * n;
	}

	inline Vector2 operator+(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs.x + rhs.x, lhs.y + rhs.y };
	}

	inline Vector2 operator-(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs.x - rhs.x, lhs.y - rhs.y };
	}

	inline Vector2 operator*(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs.x * rhs.x, lhs.y * rhs.y };
	}

	inline Vector2 operator*(const Vector2& vec, const float scalar) noexcept
	{
		return Vector2{ vec.x * scalar, vec.y * scalar };
	}

	inline Vector2 operator*(const float scalar, const Vector2& vec) noexcept
	{
		return Vector2{ vec.x * scalar, vec.y * scalar };
	}

	inline Vector2 operator/(const Vector2& lhs, const Vector2& rhs) noexcept
	{
		return Vector2{ lhs.x / rhs.x, lhs.y / rhs.y };
	}

	inline Vector2 operator/(const Vector2& vec, const float scalar) noexcept
	{
		return Vector2{ vec.x / scalar, vec.y / scalar };
	}
}