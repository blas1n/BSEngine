#pragma once

#include "Macro.h"
#include "MathFunctions.h"

class BS_API Vector2 {
public:
	static const Vector2 Zero;
	static const Vector2 One;
	static const Vector2 UnitX;
	static const Vector2 UnitY;

	float x;
	float y;

	constexpr Vector2() noexcept;
	Vector2(float inX, float inY) noexcept;
	Vector2(float* elems) noexcept;

	void Set(float inX, float inY) noexcept;

	Vector2 operator-() const noexcept;
	Vector2& operator+=(const Vector2& other) noexcept;
	Vector2& operator-=(const Vector2& other) noexcept;
	Vector2 operator*=(const Vector2& other) noexcept;
	Vector2& operator*=(const float scalar) noexcept;
	Vector2 operator/=(const Vector2& other) noexcept;
	Vector2& operator/=(const float scalar) noexcept;

	float LengthSquared() const noexcept;
	float Length() const noexcept;

	void Normalized() noexcept;
	static Vector2 Normalize(const Vector2& vec) noexcept;

	static float Dot(const Vector2& lhs, const Vector2& rhs) noexcept;
	static Vector2 Reflect(const Vector2& v, const Vector2& n) noexcept;
	static Vector2 Transform(const Vector2& vec, const class Matrix3& mat, float w = 1.0f) noexcept;

	// freinds
	friend Vector2 operator+(const Vector2& lhs, const Vector2& rhs) noexcept;
	friend Vector2 operator-(const Vector2& lhs, const Vector2& rhs) noexcept;
	friend Vector2 operator*(const Vector2& lhs, const Vector2& rhs) noexcept;
	friend Vector2 operator*(const Vector2& vec, const float scalar) noexcept;
	friend Vector2 operator*(const float scalar, const Vector2& vec) noexcept;
	friend Vector2 operator/(const Vector2& lhs, const Vector2& rhs) noexcept;
	friend Vector2 operator/(const Vector2& vec, const float scalar) noexcept;

private:
	/// @warning Do not use it as an operator for the underlying API.
	explicit operator const float* () const noexcept
	{
		return &x;
	}
};

constexpr Vector2::Vector2() noexcept
	: x(0.0f), y(0.0f) {}

Vector2::Vector2(float inX, float inY) noexcept
	: x(inX), y(inY) {}

Vector2::Vector2(float* elems) noexcept
	: x(elems[0]), y(elems[1]) {}

void Vector2::Set(float inX, float inY) noexcept
{
	x = inX;
	y = inY;
}

Vector2 Vector2::operator-() const noexcept
{
	return Vector2{ -x, -y };
}

Vector2& Vector2::operator+=(const Vector2& other) noexcept
{
	x += other.x;
	y += other.y;
}

Vector2& Vector2::operator-=(const Vector2& other) noexcept
{
	x -= other.x;
	y -= other.y;
}

Vector2 Vector2::operator*=(const Vector2& other) noexcept
{
	x *= other.x;
	y *= other.y;
}

Vector2& Vector2::operator*=(const float scalar) noexcept
{
	x *= scalar;
	y *= scalar;
}

Vector2 Vector2::operator/=(const Vector2& other) noexcept
{
	x /= other.x;
	y /= other.y;
}

Vector2& Vector2::operator/=(const float scalar) noexcept
{
	x /= scalar;
	y /= scalar;
}

float Vector2::LengthSquared() const noexcept
{
	return Math::Pow(x) + Math::Pow(y);
}

float Vector2::Length() const noexcept
{
	return Math::Sqrt(LengthSquared());
}

void Vector2::Normalized() noexcept
{
	*this /= Length();
}

Vector2 Vector2::Normalize(const Vector2& vec) noexcept
{
	return vec / vec.Length();
}

float Vector2::Dot(const Vector2& lhs, const Vector2& rhs) noexcept
{
	return (lhs.x * rhs.x) + (lhs.y * rhs.y);
}

Vector2 Vector2::Reflect(const Vector2& v, const Vector2& n) noexcept
{
	return v - 2.0f * Vector2::Dot(v, n) * n;
}

Vector2 Vector2::Transform(const Vector2& vec, const class Matrix3& mat, float w = 1.0f) noexcept
{

}

Vector2 operator+(const Vector2& lhs, const Vector2& rhs) noexcept
{
	return Vector2{ lhs.x + rhs.x, lhs.y + rhs.y };
}

Vector2 operator-(const Vector2& lhs, const Vector2& rhs) noexcept
{
	return Vector2{ lhs.x - rhs.x, lhs.y - rhs.y };
}

Vector2 operator*(const Vector2& lhs, const Vector2& rhs) noexcept
{
	return Vector2{ lhs.x * rhs.x, lhs.y * rhs.y };
}

Vector2 operator*(const Vector2& vec, const float scalar) noexcept
{
	return Vector2{ vec.x * scalar, vec.y * scalar };
}

Vector2 operator*(const float scalar, const Vector2& vec) noexcept
{
	return Vector2{ vec.x * scalar, vec.y * scalar };
}

Vector2 operator/(const Vector2& lhs, const Vector2& rhs) noexcept
{
	return Vector2{ lhs.x / rhs.x, lhs.y / rhs.y };
}

Vector2 operator/(const Vector2& vec, const float scalar) noexcept
{
	return Vector2{ vec.x / scalar, vec.y / scalar };
}