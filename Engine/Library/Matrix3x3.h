#pragma once

#include "Core.h"
#include <Eigen/Dense>
#include <utility>

namespace BE::Math
{
	class BS_API Matrix3x3 final
	{
	public:
		Matrix3x3() noexcept : mat{ } {}

		Matrix3x3(const Matrix3x3& other) noexcept : mat{ other.mat } {}
		Matrix3x3(Matrix3x3&& other) noexcept : mat{ Move(other.mat) } {}

		explicit Matrix3x3(const float* elems) noexcept
			: mat{ elems } {}

		explicit Matrix3x3(float r0c0, float r0c1, float r0c2,
			float r1c0, float r1c1, float r1c2,
			float r2c0, float r2c1, float r2c2) noexcept
			: mat{ }
		{
			mat << r0c0, r0c1, r0c2, r1c0, r1c1, r1c2, r2c0, r2c1, r2c2;
		}

		~Matrix3x3() = default;

		inline Matrix3x3& operator=(const Matrix3x3& other) noexcept
		{
			mat = other.mat;
			return *this;
		}

		inline Matrix3x3& operator=(Matrix3x3&& other) noexcept
		{
			mat = Move(other.mat);
			return *this;
		}

		inline Matrix3x3& operator+=(const Matrix3x3& other) noexcept
		{
			mat += other.mat;
			return *this;
		}

		inline Matrix3x3& operator-=(const Matrix3x3& other) noexcept
		{
			mat -= other.mat;
			return *this;
		}

		inline Matrix3x3& operator*=(const float scaler) noexcept
		{
			mat *= scaler;
			return *this;
		}

		inline Matrix3x3& operator/=(const float scaler) noexcept
		{
			mat /= scaler;
			return *this;
		}

		inline Matrix3x3& operator*=(const Matrix3x3& other) noexcept
		{
			mat *= other.mat;
			return *this;
		}

		inline float* operator[](const Uint8 row) noexcept
		{
			return &mat(row, 0);
		}

		inline const float* operator[](const Uint8 row) const noexcept
		{
			return &mat(row, 0);
		}

		inline Matrix3x3 Transpose() const noexcept
		{
			Matrix3x3 ret;
			ret.mat = mat.transpose();
			return ret;
		}
		
		inline void Transposed() noexcept
		{
			mat.transposeInPlace();
		}

		inline Matrix3x3 Invert() const noexcept
		{
			Matrix3x3 ret;
			ret.mat = mat.inverse();
			return ret;
		}

		inline void Inverted() noexcept
		{
			mat = mat.inverse();
		}

		inline float* Data() noexcept { return mat.data(); }
		inline const float* Data() const noexcept { return mat.data(); }

		static Matrix3x3 FromScale(const class Vector2& scale);
		static Matrix3x3 FromPosition(const Vector2& pos);
		static Matrix3x3 FromRotation(float angle);

		static inline Matrix3x3 Zero() noexcept
		{
			Matrix3x3 ret;
			ret.mat = Eigen::Matrix3f::Zero();
			return ret;
		}

		static inline Matrix3x3 One() noexcept
		{
			Matrix3x3 ret;
			ret.mat = Eigen::Matrix3f::Ones();
			return ret;
		}

		static inline Matrix3x3 Identity() noexcept
		{
			Matrix3x3 ret;
			ret.mat = Eigen::Matrix3f::Identity();
			return ret;
		}

	private:
		friend bool operator==(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept;

		Eigen::Matrix3f mat;
	};

	inline bool operator==(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
	{
		return lhs.mat == rhs.mat;
	}

	inline bool operator!=(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
	{
		return !(lhs == rhs);
	}

	inline Matrix3x3 operator+(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Matrix3x3 operator-(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
	{
		auto ret = lhs;
		return ret -= rhs;
	}

	inline Matrix3x3 operator*(const Matrix3x3& mat, const float scaler) noexcept
	{
		auto ret = mat;
		return ret *= scaler;
	}

	inline Matrix3x3 operator*(const float scaler, const Matrix3x3& mat) noexcept
	{
		auto ret = mat;
		return ret *= scaler;
	}

	inline Matrix3x3 operator/(const Matrix3x3& mat, float scaler) noexcept
	{
		auto ret = mat;
		return ret /= scaler;
	}

	inline Matrix3x3 operator*(const Matrix3x3& lhs, const Matrix3x3& rhs) noexcept
	{
		auto ret = lhs;
		return ret *= rhs;
	}
}