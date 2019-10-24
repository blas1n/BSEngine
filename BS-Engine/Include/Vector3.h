#pragma once

#include "Macro.h"
#include "MathFunctions.h"

class BS_API Vector3 {
public:
	float x;
	float y;
	float z;

	constexpr Vector3() noexcept;
	constexpr Vector3(float inX, float inY, float inZ) noexcept;
	Vector3(float* elems) noexcept;

	void Set(float inX, float inY, float inZ) noexcept;

	Vector3 operator-() const noexcept;
	Vector3& operator+=(const Vector3& other) noexcept;
	Vector3& operator-=(const Vector3& other) noexcept;
	Vector3 operator*=(const Vector3& other) noexcept;
	Vector3& operator*=(const float scalar) noexcept;
	Vector3 operator/=(const Vector3& other) noexcept;
	Vector3& operator/=(const float scalar) noexcept;

	float LengthSquared() const noexcept;
	float Length() const noexcept;

	void Normalized() noexcept;
	static Vector3 Normalize(const Vector3& vec) noexcept;

	static float Dot(const Vector3& lhs, const Vector3& rhs) noexcept;
	static Vector3 Cross(const Vector3& lhs, const Vector3& rhs) noexcept;
	static Vector3 Reflect(const Vector3& v, const Vector3& n) noexcept;
	static Vector3 Transform(const Vector3& vec, const class Matrix3& mat, float w = 1.0f) noexcept;

	// freinds
	friend Vector3 operator+(const Vector3& lhs, const Vector3& rhs) noexcept;
	friend Vector3 operator-(const Vector3& lhs, const Vector3& rhs) noexcept;
	friend Vector3 operator*(const Vector3& lhs, const Vector3& rhs) noexcept;
	friend Vector3 operator*(const Vector3& vec, const float scalar) noexcept;
	friend Vector3 operator*(const float scalar, const Vector3& vec) noexcept;
	friend Vector3 operator/(const Vector3& lhs, const Vector3& rhs) noexcept;
	friend Vector3 operator/(const Vector3& vec, const float scalar) noexcept;

private:
	/// @warning Do not use it as an operator for the underlying API.
	operator const float* () const noexcept
	{
		return &x;
	}
};

constexpr Vector3::Vector3() noexcept
	: x(0.0f), y(0.0f), z(0.0f) {}

constexpr Vector3::Vector3(float inX, float inY, float inZ) noexcept
	: x(inX), y(inY), z(inZ) {}

Vector3::Vector3(float* elems) noexcept
	: x(elems[0]), y(elems[1]), z(elems[2]) {}

void Vector3::Set(float inX, float inY, float inZ) noexcept
{
	x = inX;
	y = inY;
	z = inZ;
}

Vector3 Vector3::operator-() const noexcept
{
	return Vector3{ -x, -y, -z };
}

Vector3& Vector3::operator+=(const Vector3& other) noexcept
{
	x += other.x;
	y += other.y;
	z += other.z;
}

Vector3& Vector3::operator-=(const Vector3& other) noexcept
{
	x -= other.x;
	y -= other.y;
	z -= other.z;
}

Vector3 Vector3::operator*=(const Vector3& other) noexcept
{
	x *= other.x;
	y *= other.y;
	z *= other.z;
}

Vector3& Vector3::operator*=(const float scalar) noexcept
{
	x *= scalar;
	y *= scalar;
	z *= scalar;
}

Vector3 Vector3::operator/=(const Vector3& other) noexcept
{
	x /= other.x;
	y /= other.y;
	z /= other.z;
}

Vector3& Vector3::operator/=(const float scalar) noexcept
{
	x /= scalar;
	y /= scalar;
	z += scalar;
}

float Vector3::LengthSquared() const noexcept
{
	return Math::Pow(x) + Math::Pow(y) + Math::Pow(z);
}

float Vector3::Length() const noexcept
{
	return Math::Sqrt(LengthSquared());
}

void Vector3::Normalized() noexcept
{
	*this /= Length();
}

Vector3 Vector3::Normalize(const Vector3& vec) noexcept
{
	return vec / vec.Length();
}

float Vector3::Dot(const Vector3& lhs, const Vector3& rhs) noexcept
{
	return (lhs.x * rhs.x) + (lhs.y * rhs.y) + (lhs.z * rhs.z);
}

Vector3 Vector3::Cross(const Vector3& lhs, const Vector3& rhs) noexcept
{
	return Vector3{
		lhs.y * rhs.z - lhs.z * rhs.y,
		lhs.z * rhs.x - lhs.x * rhs.z,
		lhs.x * rhs.y - lhs.y * rhs.x
	};
}

Vector3 Vector3::Reflect(const Vector3& v, const Vector3& n) noexcept
{
	return v - 2.0f * Vector3::Dot(v, n) * n;
}

Vector3 Vector3::Transform(const Vector3& vec, const class Matrix3& mat, float w = 1.0f) noexcept
{

}

Vector3 operator+(const Vector3& lhs, const Vector3& rhs) noexcept
{
	return Vector3{ lhs.x + rhs.x, lhs.y + rhs.y, lhs.z + rhs.z };
}

Vector3 operator-(const Vector3& lhs, const Vector3& rhs) noexcept
{
	return Vector3{ lhs.x - rhs.x, lhs.y - rhs.y, lhs.z - rhs.z };
}

Vector3 operator*(const Vector3& lhs, const Vector3& rhs) noexcept
{
	return Vector3{ lhs.x * rhs.x, lhs.y * rhs.y, lhs.z * rhs.z };
}

Vector3 operator*(const Vector3& vec, const float scalar) noexcept
{
	return Vector3{ vec.x * scalar, vec.y * scalar, vec.z * scalar };
}

Vector3 operator*(const float scalar, const Vector3& vec) noexcept
{
	return Vector3{ vec.x * scalar, vec.y * scalar, vec.z * scalar };
}

Vector3 operator/(const Vector3& lhs, const Vector3& rhs) noexcept
{
	return Vector3{ lhs.x / rhs.x, lhs.y / rhs.y, lhs.z / rhs.z };
}

Vector3 operator/(const Vector3& vec, const float scalar) noexcept
{
	return Vector3{ vec.x / scalar, vec.y / scalar, vec.z / scalar };
}