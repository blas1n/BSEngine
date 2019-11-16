#pragma once

#include "Vector3.h"
#include "Vector4.h"

/// @todo Use SIMD register.
class BS_API Matrix4x4
{
public:
	static const Matrix4x4 Zero;
	static const Matrix4x4 One;
	static const Matrix4x4 Identity;

	constexpr Matrix4x4() noexcept;
	Matrix4x4(Vector4 inRows[4]) noexcept;
	Matrix4x4(const Vector4& row0, const Vector4& row1, const Vector4& row2, const Vector4& row3) noexcept;
	Matrix4x4(float elems[4][4]) noexcept;
	Matrix4x4(float r0c0, float r0c1, float r0c2, float r0c3,
		float r1c0, float r1c1, float r1c2, float r1c3,
		float r2c0, float r2c1, float r2c2, float r2c3,
		float r3c0, float r3c1, float r3c2, float r3c3) noexcept;

	Matrix4x4(const Matrix4x4& other) noexcept = default;
	Matrix4x4(Matrix4x4&& other) noexcept = default;

	Matrix4x4& operator=(const Matrix4x4& other) noexcept = default;
	Matrix4x4& operator=(Matrix4x4&& other) noexcept = default;

	~Matrix4x4() noexcept = default;

	bool operator==(const Matrix4x4& other) const noexcept;
	bool operator!=(const Matrix4x4& other) const noexcept;

	Matrix4x4& operator+=(const Matrix4x4& other) noexcept;
	Matrix4x4& operator-=(const Matrix4x4& other) noexcept;

	Matrix4x4& operator*=(float scaler) noexcept;
	Matrix4x4& operator/=(float scaler) noexcept;

	Matrix4x4& operator*=(const Matrix4x4& other) noexcept;

	friend Matrix4x4 operator+(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept;
	friend Matrix4x4 operator-(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept;

	friend Matrix4x4 operator*(const Matrix4x4& mat, float scaler) noexcept;
	friend Matrix4x4 operator*(float scaler, const Matrix4x4& mat) noexcept;

	friend Matrix4x4 operator/(const Matrix4x4& mat, float scaler) noexcept;

	friend Matrix4x4 operator*(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept;

	Matrix4x4 operator+() const noexcept;
	Matrix4x4 operator-() const noexcept;

	Vector4& operator[](uint8 row) noexcept;
	const Vector4& operator[](uint8 row) const noexcept;

	static Matrix4x4 FromTranslation(float x, float y, float z) noexcept;
	static Matrix4x4 FromTranslation(const Vector3& translation) noexcept;

	static Matrix4x4 FromRotationX(float theta) noexcept;
	static Matrix4x4 FromRotationY(float theta) noexcept;
	static Matrix4x4 FromRotationZ(float theta) noexcept;

	static Matrix4x4 FromQuaternion(const class Quaternion& q) noexcept;

	static Matrix4x4 FromScale(float x, float y, float z) noexcept;
	static Matrix4x4 FromScale(const Vector3& scale) noexcept;
	static Matrix4x4 FromScale(float scale) noexcept;

	Vector3 GetTranslation() const noexcept;
	Vector3 GetScale() const noexcept;

	static Matrix4x4 Transpose(const Matrix4x4& mat) noexcept;

	static Matrix4x4 Invert(const Matrix4x4& mat) noexcept;
	void Inverted() noexcept;

	/// @warning Do not use it as an operator for the underlying API.
	explicit operator const float* () const noexcept
	{
		return reinterpret_cast<const float*>(rows);
	}

private:
	Vector4 rows[4];
};

inline constexpr Matrix4x4::Matrix4x4() noexcept
	: rows() {}

inline Matrix4x4::Matrix4x4(Vector4 inRows[4]) noexcept
	: rows()
{
	rows[0] = inRows[0];
	rows[1] = inRows[1];
	rows[2] = inRows[2];
	rows[3] = inRows[3];
}

inline Matrix4x4::Matrix4x4(const Vector4& row0, const Vector4& row1, const Vector4& row2, const Vector4& row3) noexcept
	: rows()
{
	rows[0] = row0;
	rows[1] = row1;
	rows[2] = row2;
	rows[3] = row3;
}

inline Matrix4x4::Matrix4x4(float elems[4][4]) noexcept
	: rows()
{
	rows[0].Set(elems[0][0], elems[0][1], elems[0][2], elems[0][3]);
	rows[1].Set(elems[1][0], elems[1][1], elems[1][2], elems[1][3]);
	rows[2].Set(elems[2][0], elems[2][1], elems[2][2], elems[2][3]);
	rows[3].Set(elems[3][0], elems[3][1], elems[3][2], elems[3][3]);
}

inline Matrix4x4::Matrix4x4(
	float r0c0, float r0c1, float r0c2, float r0c3,
	float r1c0, float r1c1, float r1c2, float r1c3,
	float r2c0, float r2c1, float r2c2, float r2c3,
	float r3c0, float r3c1, float r3c2, float r3c3) noexcept
	: rows()
{
	rows[0].Set(r0c0, r0c1, r0c2, r0c3);
	rows[1].Set(r1c0, r1c1, r1c2, r1c3);
	rows[2].Set(r2c0, r2c1, r2c2, r2c3);
	rows[3].Set(r3c0, r3c1, r3c2, r3c3);
}

inline bool Matrix4x4::operator==(const Matrix4x4& other) const noexcept
{
	for (uint8 row = 0; row < 4; ++row)
		for (uint8 column = 0; column < 4; ++column)
			if (rows[row][column] != other.rows[row][column])
				return false;

	return true;
}

inline bool Matrix4x4::operator!=(const Matrix4x4& other) const noexcept
{
	return !(*this == other);
}

inline Matrix4x4& Matrix4x4::operator+=(const Matrix4x4& other) noexcept
{
	rows[0] += other[0];
	rows[1] += other[1];
	rows[2] += other[2];
	rows[3] += other[3];
	return *this;
}

inline Matrix4x4& Matrix4x4::operator-=(const Matrix4x4& other) noexcept
{
	rows[0] -= other[0];
	rows[1] -= other[1];
	rows[2] -= other[2];
	rows[3] -= other[3];
	return *this;
}

inline Matrix4x4& Matrix4x4::operator*=(float scaler) noexcept
{
	rows[0] *= scaler;
	rows[1] *= scaler;
	rows[2] *= scaler;
	rows[3] *= scaler;
	return *this;
}

inline Matrix4x4& Matrix4x4::operator/=(float scaler) noexcept
{
	rows[0] /= scaler;
	rows[1] /= scaler;
	rows[2] /= scaler;
	rows[3] /= scaler;
	return *this;
}

inline Matrix4x4& Matrix4x4::operator*=(const Matrix4x4& other) noexcept
{
	*this = *this * other;
	return *this;
}

inline Matrix4x4 operator+(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept
{
	return Matrix4x4
	{
		lhs[0] + rhs[0],
		lhs[1] + rhs[1],
		lhs[2] + rhs[2],
		lhs[3] + rhs[3]
	};
}

inline Matrix4x4 operator-(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept
{
	return Matrix4x4
	{
		lhs[0] - rhs[0],
		lhs[1] - rhs[1],
		lhs[2] - rhs[2],
		lhs[3] - rhs[3]
	};
}

inline Matrix4x4 operator*(const Matrix4x4& mat, float scaler) noexcept
{
	return Matrix4x4
	{
		mat[0] * scaler,
		mat[1] * scaler,
		mat[2] * scaler,
		mat[3] * scaler
	};
}

inline Matrix4x4 operator*(float scaler, const Matrix4x4& mat) noexcept
{
	return Matrix4x4
	{
		mat[0] * scaler,
		mat[1] * scaler,
		mat[2] * scaler,
		mat[3] * scaler
	};
}

inline Matrix4x4 operator/(const Matrix4x4& mat, float scaler) noexcept
{
	return Matrix4x4
	{
		mat[0] / scaler,
		mat[1] / scaler,
		mat[2] / scaler,
		mat[3] / scaler,
	};
}

inline Matrix4x4 Matrix4x4::operator+() const noexcept
{
	return *this;
}

inline Matrix4x4 Matrix4x4::operator-() const noexcept
{
	return *this * -1.0f;
}

inline Vector4& Matrix4x4::operator[](uint8 row) noexcept
{
	return rows[row];
}

inline const Vector4& Matrix4x4::operator[](uint8 row) const noexcept
{
	return rows[row];
}

inline Matrix4x4 Matrix4x4::FromTranslation(float x, float y, float z) noexcept
{
	return Matrix4x4
	{
		1.0f, 0.0, 0.0f, 0.0f,
		0.0f, 1.0, 0.0f, 0.0f,
		0.0f, 0.0, 1.0f, 0.0f,
		x, y, z, 1.0f
	};
}

inline Matrix4x4 Matrix4x4::FromTranslation(const Vector3& translation) noexcept
{
	return FromTranslation(translation.x, translation.y, translation.z);
}

inline Matrix4x4 Matrix4x4::FromScale(float x, float y, float z) noexcept
{
	return Matrix4x4
	{
		x, 0.0f, 0.0f, 0.0f,
		0.0f, y, 0.0f, 0.0f,
		0.0f, 0.0f, z, 0.0f,
		0.0f, 0.0f, 0.0f, 1.0f
	};
}

inline Matrix4x4 Matrix4x4::FromScale(const Vector3& scale) noexcept
{
	return FromScale(scale.x, scale.y, scale.z);
}

inline Matrix4x4 Matrix4x4::FromScale(float scale) noexcept
{
	return FromScale(scale, scale, scale);
}

inline Vector3 Matrix4x4::GetTranslation() const noexcept
{
	return Vector3{ rows[3][0], rows[3][1], rows[3][2] };
}

inline Vector3 Matrix4x4::GetScale() const noexcept
{
	return Vector3
	{
		Vector3{ rows[0][0], rows[0][1], rows[0][2] }.Length(),
		Vector3{ rows[1][0], rows[1][1], rows[1][2] }.Length(),
		Vector3{ rows[2][0], rows[2][1], rows[2][2] }.Length()
	};
}

inline Matrix4x4 Matrix4x4::Transpose(const Matrix4x4& mat) noexcept
{
	return Matrix4x4
	{
		mat[0][0], mat[1][0], mat[2][0], mat[3][0],
		mat[0][1], mat[1][1], mat[2][1], mat[3][1],
		mat[0][2], mat[1][2], mat[2][2], mat[3][2],
		mat[0][3], mat[1][3], mat[2][3], mat[3][3]
	};
}

inline Matrix4x4 Matrix4x4::Invert(const Matrix4x4& mat) noexcept
{
	auto ret = mat;
	ret.Inverted();
	return ret;
}