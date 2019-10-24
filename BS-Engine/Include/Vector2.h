#pragma once

#include "Macro.h"

class BS_API Vector2 {
public:
	float x;
	float y;

	Vector2() noexcept;
	Vector2(float inX, float inY) noexcept;
	Vector2(float* elems) noexcept;

	void Set(float inX, float inY) noexcept;

	Vector2 operator-() const noexcept;
	Vector2& operator+=(const Vector2& other);
	Vector2& operator-=(const Vector2& other);
	Vector2 operator*=(const Vector2& other);
	Vector2& operator*=(const float scalar);
	Vector2 operator/=(const Vector2& other);
	Vector2& operator/=(const float scalar);

	float LengthSquared() const;
	float Length() const;

	void Normalized();
	static Vector2 Normalize(const Vector2& vec);

	static float Dot(const Vector2& lhs, const Vector2& rhs);
	static Vector2 Reflect(const Vector2& v, const Vector2& n);
	static Vector2 Transform(const Vector2& vec, const class Matrix3& mat, float w = 1.0f);

	// freinds
	friend Vector2 operator+(const Vector2& lhs, const Vector2& rhs);
	friend Vector2 operator-(const Vector2& lhs, const Vector2& rhs);
	friend Vector2 operator*(const Vector2& lhs, const Vector2& rhs);
	friend Vector2 operator*(const Vector2& vec, const float scalar);
	friend Vector2 operator*(const float scalar, const Vector2& vec);
	friend Vector2 operator/(const Vector2& lhs, const Vector2& rhs);
	friend Vector2 operator/(const Vector2& vec, const float scalar);
};