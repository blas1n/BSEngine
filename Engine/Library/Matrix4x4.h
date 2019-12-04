#pragma once

#include "Core.h"
#include <Eigen/Dense>
#include <utility>

namespace BE::Math
{
	class BS_API Matrix4x4 final
	{
	public:
		Matrix4x4() noexcept : mat{ } {}

		Matrix4x4(const Matrix4x4& other) noexcept : mat{ other.mat } {}
		Matrix4x4(Matrix4x4&& other) noexcept : mat{ std::move(other.mat) } {}

		explicit Matrix4x4(const float* elems) noexcept
			: mat{ elems } {}

		explicit Matrix4x4(float r0c0, float r0c1, float r0c2, float r0c3,
			float r1c0, float r1c1, float r1c2, float r1c3,
			float r2c0, float r2c1, float r2c2, float r2c3,
			float r3c0, float r3c1, float r3c2, float r3c3) noexcept
			: mat{ }
		{
			mat << r0c0, r0c1, r0c2, r0c3,
				r1c0, r1c1, r1c2, r1c3,
				r2c0, r2c1, r2c2, r2c3,
				r3c0, r3c1, r3c2, r3c3;
		}

		~Matrix4x4() = default;

		Matrix4x4& operator=(const Matrix4x4& other) noexcept
		{
			mat = other.mat;
			return *this;
		}

		Matrix4x4& operator=(Matrix4x4&& other) noexcept
		{
			mat = std::move(other.mat);
			return *this;
		}

		inline bool operator==(const Matrix4x4& other) const noexcept
		{
			return mat == other.mat;
		}

		inline bool operator!=(const Matrix4x4& other) const noexcept
		{
			return mat != other.mat;
		}

		inline Matrix4x4& operator+=(const Matrix4x4& other) noexcept
		{
			mat += other.mat;
			return *this;
		}

		inline Matrix4x4& operator-=(const Matrix4x4& other) noexcept
		{
			mat -= other.mat;
			return *this;
		}

		inline Matrix4x4& operator*=(const float scaler) noexcept
		{
			mat *= scaler;
			return *this;
		}

		inline Matrix4x4& operator/=(const float scaler) noexcept
		{
			mat /= scaler;
			return *this;
		}

		inline Matrix4x4& operator*=(const Matrix4x4& other) noexcept
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

		inline Matrix4x4 Transpose() const noexcept
		{
			Matrix4x4 ret;
			ret.mat = mat.transpose();
			return ret;
		}

		inline void Transposed() noexcept
		{
			mat.transposeInPlace();
		}

		inline Matrix4x4 Invert() const noexcept
		{
			Matrix4x4 ret;
			ret.mat = mat.inverse();
			return ret;
		}

		inline void Inverted() noexcept
		{
			mat = mat.inverse();
		}

		static Matrix4x4 FromScale(const class Vector3& scale);
		static Matrix4x4 FromPosition(const Vector3& pos);
		static Matrix4x4 FromRotation(const class Rotator& angle);

	private:
		Eigen::Matrix4f mat;
	};

	inline Matrix4x4 operator+(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept
	{
		auto ret = lhs;
		return ret += rhs;
	}

	inline Matrix4x4 operator-(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept
	{
		auto ret = lhs;
		return ret -= rhs;
	}

	inline Matrix4x4 operator*(const Matrix4x4& mat, const float scaler) noexcept
	{
		auto ret = mat;
		return ret *= scaler;
	}

	inline Matrix4x4 operator*(const float scaler, const Matrix4x4& mat) noexcept
	{
		auto ret = mat;
		return ret *= scaler;
	}

	inline Matrix4x4 operator/(const Matrix4x4& mat, float scaler) noexcept
	{
		auto ret = mat;
		return ret /= scaler;
	}

	inline Matrix4x4 operator*(const Matrix4x4& lhs, const Matrix4x4& rhs) noexcept
	{
		auto ret = lhs;
		return ret *= rhs;
	}
}