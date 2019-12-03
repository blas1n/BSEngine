#pragma once

#include "Core.h"
#include <Eigen/Dense>

namespace BE::Math
{
	class BS_API Matrix3x3
	{
	public:
		Matrix3x3() noexcept : mat{ } {}

		Matrix3x3(const Matrix3x3& other) noexcept : mat{ other.mat } {}
		Matrix3x3(Matrix3x3&& other) noexcept : mat{ std::move(other.mat) } {}

		Matrix3x3& operator=(const Matrix3x3& other) noexcept { mat = other.mat; }
		Matrix3x3& operator=(Matrix3x3&& other) noexcept { mat = std::move(other.mat); }

		~Matrix3x3() = default;

		explicit Matrix3x3(const float* elems) noexcept
			: mat{ elems } {}

		explicit Matrix3x3(float r0c0, float r0c1, float r0c2,
			float r1c0, float r1c1, float r1c2,
			float r2c0, float r2c1, float r2c2) noexcept
			: mat{ }
		{
			mat << r0c0, r0c1, r0c2, r1c0, r1c1, r1c2, r2c0, r2c1, r2c2;
		}

		inline bool operator==(const Matrix3x3& other) const noexcept
		{
			return mat == other.mat;
		}

		inline bool operator!=(const Matrix3x3& other) const noexcept
		{
			return mat != other.mat;
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

	private:
		Eigen::Matrix3f mat;
	};

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