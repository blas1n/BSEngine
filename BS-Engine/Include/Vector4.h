#pragma once

#include "Macro.h"
#include "MathFunctions.h"

class BS_API Vector4 {
public:
	static const Vector4 Zero;
	static const Vector4 One;
	static const Vector4 UnitX;
	static const Vector4 UnitY;
	static const Vector4 UnitZ;
	static const Vector4 UnitW;

	float x;
	float y;
	float z;
	float w;

	constexpr Vector4() noexcept;
	constexpr Vector4(float inX, float inY, float inZ, float inW) noexcept;
	Vector4(float* elems) noexcept;

	void Set(float inX, float inY, float inZ, float inW) noexcept;

	Vector4 operator-() const noexcept;
	Vector4& operator+=(const Vector4& other) noexcept;
	Vector4& operator-=(const Vector4& other) noexcept;
	Vector4 operator*=(const Vector4& other) noexcept;
	Vector4& operator*=(const float scalar) noexcept;
	Vector4 operator/=(const Vector4& other) noexcept;
	Vector4& operator/=(const float scalar) noexcept;

	float LengthSquared() const noexcept;
	float Length() const noexcept;

	void Normalized() noexcept;
	static Vector4 Normalize(const Vector4& vec) noexcept;

	static float Dot(const Vector4& lhs, const Vector4& rhs) noexcept;
	static Vector4 Reflect(const Vector4& v, const Vector4& n) noexcept;
	static Vector4 Transform(const Vector4& vec, const class Matrix3& mat, float w = 1.0f) noexcept;

	// freinds
	friend Vector4 operator+(const Vector4& lhs, const Vector4& rhs) noexcept;
	friend Vector4 operator-(const Vector4& lhs, const Vector4& rhs) noexcept;
	friend Vector4 operator*(const Vector4& lhs, const Vector4& rhs) noexcept;
	friend Vector4 operator*(const Vector4& vec, const float scalar) noexcept;
	friend Vector4 operator*(const float scalar, const Vector4& vec) noexcept;
	friend Vector4 operator/(const Vector4& lhs, const Vector4& rhs) noexcept;
	friend Vector4 operator/(const Vector4& vec, const float scalar) noexcept;

private:
	/// @warning Do not use it as an operator for the underlying API.
	operator const float*() const noexcept
	{
		return &x;
	}
};

constexpr Vector4::Vector4() noexcept
	: x(0.0f), y(0.0f), z(0.0f), w(0.0f) {}

constexpr Vector4::Vector4(const float inX, const float inY, const float inZ, const float inW) noexcept
	: x(inX), y(inY), z(inZ), w(inW) {}

Vector4::Vector4(float* elems) noexcept
	: x(elems[0]), y(elems[1]), z(elems[2]), w(elems[3]) {}

void Vector4::Set(const float inX, const float inY, const float inZ, const float inW) noexcept
{
	x = inX;
	y = inY;
	z = inZ;
	w = inW;
}

Vector4 Vector4::operator-() const noexcept
{
	return Vector4{ -x, -y, -z, -w };
}

Vector4& Vector4::operator+=(const Vector4& other) noexcept
{
	x += other.x;
	y += other.y;
	z += other.z;
	w += other.w;
}

Vector4& Vector4::operator-=(const Vector4& other) noexcept
{
	x -= other.x;
	y -= other.y;
	z -= other.z;
	w -= other.w;
}

Vector4 Vector4::operator*=(const Vector4& other) noexcept
{
	x *= other.x;
	y *= other.y;
	z *= other.z;
	w *= other.w;
}

Vector4& Vector4::operator*=(const float scalar) noexcept
{
	x *= scalar;
	y *= scalar;
	z *= scalar;
	w *= scalar;
}

Vector4 Vector4::operator/=(const Vector4& other) noexcept
{
	x /= other.x;
	y /= other.y;
	z /= other.z;
	w /= other.w;
}

Vector4& Vector4::operator/=(const float scalar) noexcept
{
	x /= scalar;
	y /= scalar;
	z /= scalar;
	w /= scalar;
}

float Vector4::LengthSquared() const noexcept
{
	return Math::Pow(x) + Math::Pow(y) + Math::Pow(z) + Math::Pow(w);
}

float Vector4::Length() const noexcept
{
	return Math::Sqrt(LengthSquared());
}

void Vector4::Normalized() noexcept
{
	*this /= Length();
}

Vector4 Vector4::Normalize(const Vector4& vec) noexcept
{
	return vec / vec.Length();
}

float Vector4::Dot(const Vector4& lhs, const Vector4& rhs) noexcept
{
	return (lhs.x * rhs.x) + (lhs.y * rhs.y) + (lhs.z * rhs.z) + (lhs.w * rhs.w);
}

Vector4 Vector4::Reflect(const Vector4& v, const Vector4& n) noexcept
{
	return v - 2.0f * Vector4::Dot(v, n) * n;
}

Vector4 Vector4::Transform(const Vector4& vec, const class Matrix3& mat, float w = 1.0f) noexcept
{

}

Vector4 operator+(const Vector4& lhs, const Vector4& rhs) noexcept
{
	return Vector4{ lhs.x + rhs.x, lhs.y + rhs.y, lhs.z + rhs.z, lhs.w + rhs.w };
}

Vector4 operator-(const Vector4& lhs, const Vector4& rhs) noexcept
{
	return Vector4{ lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z, lhs.w - rhs.w };
}

Vector4 operator*(const Vector4& lhs, const Vector4& rhs) noexcept
{
	return Vector4{ lhs.x * rhs.x, lhs.y * rhs.y, lhs.z * rhs.z, lhs.w * rhs.w };
}

Vector4 operator*(const Vector4& vec, const float scalar) noexcept
{
	return Vector4{ vec.x * scalar, vec.y * scalar, vec.z * scalar, vec.w * scalar };
}

Vector4 operator*(const float scalar, const Vector4& vec) noexcept
{
	return Vector4{ vec.x * scalar, vec.y * scalar, vec.z * scalar, vec.w * scalar };
}

Vector4 operator/(const Vector4& lhs, const Vector4& rhs) noexcept
{
	return Vector4{ lhs.x / rhs.x, lhs.y / rhs.y, lhs.z / rhs.z, lhs.w / rhs.w };
}

Vector4 operator/(const Vector4& vec, const float scalar) noexcept
{
	return Vector4{ vec.x / scalar, vec.y / scalar, vec.z / scalar, vec.w / scalar };
}