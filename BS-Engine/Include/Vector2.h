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
};