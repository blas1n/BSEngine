#pragma once

#include "Vector2.h"
#include "Vector3.h"

namespace BE::Math
{
	/// @todo Use SIMD register.
	class BS_API Matrix3x3
	{
	public:
		static const Matrix3x3 Zero;
		static const Matrix3x3 One;
		static const Matrix3x3 Identity;

		constexpr Matrix3x3() noexcept;
		Matrix3x3(Vector3 inRows[3]) noexcept;
		Matrix3x3(const Vector3& row0, const Vector3& row1, const Vector3& row2) noexcept;
		Matrix3x3(float elems[3][3]) noexcept;
		Matrix3x3(float r0c0, float r0c1, float r0c2,
			float r1c0, float r1c1, float r1c2,
			float r2c0, float r2c1, float r2c2) noexcept;

		Matrix3x3(const Matrix3x3& other) noexcept = default;
		Matrix3x3(Matrix3x3&& other) noexcept = default;

		Matrix3x3& operator=(const Matrix3x3& other) noexcept = default;
		Matrix3x3& operator=(Matrix3x3&& other) noexcept = default;

		~Matrix3x3() noexcept = default;

		bool operator==(const Matrix3x3& other) const noexcept;
		bool operator!=(const Matrix3x3& other) const noexcept;

		Matrix3x3& operator+=(const Matrix3x3& other) noexcept;
		Matrix3x3& operator-=(const Matrix3x3& other) noexcept;

		Matrix3x3& operator*=(float scaler) noexcept;
		Matrix3x3& operator/=(float scaler) noexcept;

		Matrix3x3& operator*=(const Matrix3x3& other) noexcept;

		friend Matrix3x3 operator+(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept;
		friend Matrix3x3 operator-(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept;

		friend Matrix3x3 operator*(const Matrix3x3& mat, float scaler) noexcept;
		friend Matrix3x3 operator*(float scaler, const Matrix3x3& mat) noexcept;

		friend Matrix3x3 operator/(const Matrix3x3& mat, float scaler) noexcept;

		friend Matrix3x3 operator*(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept;

		Matrix3x3 operator+() const noexcept;
		Matrix3x3 operator-() const noexcept;

		Vector3& operator[](uint8 row) noexcept;
		const Vector3& operator[](uint8 row) const noexcept;

		static Matrix3x3 FromTranslation(float x, float y) noexcept;
		static Matrix3x3 FromTranslation(const Vector2& translation) noexcept;

		static Matrix3x3 FromRotation(float theta) noexcept;

		static Matrix3x3 FromScale(float x, float y) noexcept;
		static Matrix3x3 FromScale(const Vector2& scale) noexcept;
		static Matrix3x3 FromScale(float scale) noexcept;

		Vector2 GetTranslation() const noexcept;
		Vector2 GetScale() const noexcept;

		static Matrix3x3 Transpose(const Matrix3x3& mat) noexcept;

		static Matrix3x3 Invert(const Matrix3x3& mat) noexcept;
		void Inverted() noexcept;

		/// @warning Do not use it as an operator for the underlying API.
		explicit operator const float* () const noexcept
		{
			return reinterpret_cast<const float*>(rows);
		}

	private:
		Vector3 rows[3];
	};

	inline constexpr Matrix3x3::Matrix3x3() noexcept
		: rows() {}

	inline Matrix3x3::Matrix3x3(Vector3 inRows[3]) noexcept
		: rows()
	{
		rows[0] = inRows[0];
		rows[1] = inRows[1];
		rows[2] = inRows[2];
	}

	inline Matrix3x3::Matrix3x3(const Vector3& row0, const Vector3& row1, const Vector3& row2) noexcept
		: rows()
	{
		rows[0] = row0;
		rows[1] = row1;
		rows[2] = row2;
	}

	inline Matrix3x3::Matrix3x3(float elems[3][3]) noexcept
		: rows()
	{
		rows[0].Set(elems[0][0], elems[0][1], elems[0][2]);
		rows[1].Set(elems[1][0], elems[1][1], elems[1][2]);
		rows[2].Set(elems[2][0], elems[2][1], elems[2][2]);
	}

	inline Matrix3x3::Matrix3x3(
		float r0c0, float r0c1, float r0c2,
		float r1c0, float r1c1, float r1c2,
		float r2c0, float r2c1, float r2c2) noexcept
		: rows()
	{
		rows[0].Set(r0c0, r0c1, r0c2);
		rows[1].Set(r1c0, r1c1, r1c2);
		rows[2].Set(r2c0, r2c1, r2c2);
	}

	inline bool Matrix3x3::operator==(const Matrix3x3& other) const noexcept
	{
		for (uint8 row = 0; row < 3; ++row)
			for (uint8 column = 0; column < 3; ++column)
				if (rows[row][column] != other.rows[row][column])
					return false;

		return true;
	}

	inline bool Matrix3x3::operator!=(const Matrix3x3& other) const noexcept
	{
		return !(*this == other);
	}

	inline Matrix3x3& Matrix3x3::operator+=(const Matrix3x3& other) noexcept
	{
		rows[0] += other[0];
		rows[1] += other[1];
		rows[2] += other[2];
		return *this;
	}

	inline Matrix3x3& Matrix3x3::operator-=(const Matrix3x3& other) noexcept
	{
		rows[0] -= other[0];
		rows[1] -= other[1];
		rows[2] -= other[2];
		return *this;
	}

	inline Matrix3x3& Matrix3x3::operator*=(float scaler) noexcept
	{
		rows[0] *= scaler;
		rows[1] *= scaler;
		rows[2] *= scaler;
		return *this;
	}

	inline Matrix3x3& Matrix3x3::operator/=(float scaler) noexcept
	{
		rows[0] /= scaler;
		rows[1] /= scaler;
		rows[2] /= scaler;
		return *this;
	}

	inline Matrix3x3& Matrix3x3::operator*=(const Matrix3x3& other) noexcept
	{
		*this = *this * other;
		return *this;
	}

	inline Matrix3x3 operator+(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
	{
		return Matrix3x3
		{
			lhs[0] + rhs[0],
			lhs[1] + rhs[1],
			lhs[2] + rhs[2]
		};
	}

	inline Matrix3x3 operator-(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
	{
		return Matrix3x3
		{
			lhs[0] - rhs[0],
			lhs[1] - rhs[1],
			lhs[2] - rhs[2]
		};
	}

	inline Matrix3x3 operator*(const Matrix3x3& mat, float scaler) noexcept
	{
		return Matrix3x3
		{
			mat[0] * scaler,
			mat[1] * scaler,
			mat[2] * scaler
		};
	}

	inline Matrix3x3 operator*(float scaler, const Matrix3x3& mat) noexcept
	{
		return Matrix3x3
		{
			mat[0] * scaler,
			mat[1] * scaler,
			mat[2] * scaler
		};
	}

	inline Matrix3x3 operator/(const Matrix3x3& mat, float scaler) noexcept
	{
		return Matrix3x3
		{
			mat[0] / scaler,
			mat[1] / scaler,
			mat[2] / scaler
		};
	}

	inline Matrix3x3 Matrix3x3::operator+() const noexcept
	{
		return *this;
	}

	inline Matrix3x3 Matrix3x3::operator-() const noexcept
	{
		return *this * -1.0f;
	}

	inline Vector3& Matrix3x3::operator[](uint8 row) noexcept
	{
		return rows[row];
	}

	inline const Vector3& Matrix3x3::operator[](uint8 row) const noexcept
	{
		return rows[row];
	}

	inline Matrix3x3 Matrix3x3::FromTranslation(float x, float y) noexcept
	{
		return Matrix3x3
		{
			1.0f, 0.0f, 0.0f,
			0.0f, 1.0f, 0.0f,
			x, y, 1.0f
		};
	}

	inline Matrix3x3 Matrix3x3::FromTranslation(const Vector2& translation) noexcept
	{
		return FromTranslation(translation.x, translation.y);
	}

	inline Matrix3x3 Matrix3x3::FromScale(float x, float y) noexcept
	{
		return Matrix3x3
		{
			x, 0.0f, 0.0f,
			0.0f, y, 0.0f,
			0.0f, 0.0f, 1.0f
		};
	}

	inline Matrix3x3 Matrix3x3::FromScale(const Vector2& scale) noexcept
	{
		return FromScale(scale.x, scale.y);
	}

	inline Matrix3x3 Matrix3x3::FromScale(float scale) noexcept
	{
		return FromScale(scale, scale);
	}

	inline Vector2 Matrix3x3::GetTranslation() const noexcept
	{
		return Vector2{ rows[2][0], rows[2][1] };
	}

	inline Vector2 Matrix3x3::GetScale() const noexcept
	{
		return Vector2
		{
			Vector2{ rows[0][0], rows[0][1] }.Length(),
			Vector2{ rows[1][0], rows[1][1] }.Length(),
		};
	}

	inline Matrix3x3 Matrix3x3::Transpose(const Matrix3x3& mat) noexcept
	{
		return Matrix3x3
		{
			mat[0][0], mat[1][0], mat[2][0],
			mat[0][1], mat[1][1], mat[2][1],
			mat[0][2], mat[1][2], mat[2][2],
		};
	}

	inline Matrix3x3 Matrix3x3::Invert(const Matrix3x3& mat) noexcept
	{
		auto ret = mat;
		ret.Inverted();
		return ret;
	}
}