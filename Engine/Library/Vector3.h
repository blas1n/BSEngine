#pragma once

#include "Core.h"
#include <Eigen/Dense>
#include <utility>

namespace BE::Math
{
	class Vector2;
	class Vector4;

	class BS_API Vector3 final
	{
	public:
		Vector3() noexcept : vec{ } {}

		Vector3(const Vector3& other) noexcept : vec{ other.vec } {}
		Vector3(Vector3&& other) noexcept : vec{ std::move(other.vec) } {}

		explicit Vector3(const float x, const float y, const float z) noexcept
			: vec{ x, y, z } {}

		explicit Vector3(const float elems[3]) noexcept
			: vec{ elems } {}

		~Vector3() = default;

		inline void Set(const float x, const float y, const float z) noexcept
		{
			vec << x, y, z;
		}

		inline float& x() noexcept { return (*this)[0]; }
		inline float x() const noexcept { return (*this)[0]; }

		inline float& y() noexcept { return (*this)[1]; }
		inline float y() const noexcept { return (*this)[1]; }

		inline float& z() noexcept { return (*this)[2]; }
		inline float z() const noexcept { return (*this)[2]; }

		inline Vector3& operator=(const Vector3& other) noexcept
		{
			vec = other.vec;
			return *this;
		}

		inline Vector3& operator=(Vector3&& other) noexcept
		{
			vec = std::move(other.vec);
			return *this;
		}

		inline float& operator[](const Uint8 index) noexcept
		{
			return vec[index];
		}

		inline float operator[](const Uint8 index) const noexcept
		{
			return vec[index];
		}

		inline Vector3 operator-() const noexcept;

		inline bool operator==(const Vector3& other) const noexcept
		{
			return vec == other.vec;
		}

		inline bool operator!=(const Vector3& other) const noexcept
		{
			return vec != other.vec;
		}

		inline Vector3& operator+=(const Vector3& other) noexcept
		{
			vec += other.vec;
			return *this;
		}

		inline Vector3& operator-=(const Vector3& other) noexcept
		{
			vec -= other.vec;
			return *this;
		}

		inline Vector3& operator*=(const Vector3& other) noexcept
		{
			vec[0] *= other.vec[0];
			vec[1] *= other.vec[1];
			return *this;
		}

		inline Vector3& operator*=(const float scalar) noexcept
		{
			vec *= scalar;
			return *this;
		}

		inline Vector3& operator/=(const Vector3& other) noexcept
		{
			vec[0] /= other.vec[0];
			vec[1] /= other.vec[1];
			return *this;
		}

		inline Vector3& operator/=(const float scalar) noexcept
		{
			vec /= scalar;
			return *this;
		}

		inline float LengthSquared() const noexcept
		{
			return vec.squaredNorm();
		}

		inline float Length() const noexcept
		{
			return vec.norm();
		}

		inline void Normalize() noexcept
		{
			vec.normalize();
		}

		inline Vector3 Normalized() const noexcept
		{
			Vector3 ret;
			ret.vec = vec.normalized();
			return ret;
		}

		inline float* Data() noexcept { return vec.data(); }
		inline const float* Data() const noexcept { return vec.data(); }

		static inline float Dot(const Vector3& lhs, const Vector3& rhs) noexcept
		{
			return lhs.vec.dot(rhs.vec);
		}

		static inline Vector3 Cross(const Vector3& lhs, const Vector3& rhs) noexcept
		{
			Vector3 ret;
			ret.vec = lhs.vec.cross(rhs.vec);
			return ret;
		}

		static inline Vector3 Reflect(const Vector3& v, const Vector3& n) noexcept;

		static inline Vector3 Zero() noexcept
		{
			Vector3 ret;
			ret.vec = Eigen::Vector3f::UnitX();
			return ret;
		}

		static inline Vector3 One() noexcept
		{
			Vector3 ret;
			ret.vec = Eigen::Vector3f::Ones();
			return ret;
		}

		static inline Vector3 UnitX() noexcept
		{
			Vector3 ret;
			ret.vec = Eigen::Vector3f::UnitX();
			return ret;
		}

		static inline Vector3 UnitY() noexcept
		{
			Vector3 ret;
			ret.vec = Eigen::Vector3f::UnitY();
			return ret;
		}

		static inline Vector3 UnitZ() noexcept
		{
			Vector3 ret;
			ret.vec = Eigen::Vector3f::UnitZ();
			return ret;
		}

		explicit operator Vector2() const noexcept;
		explicit operator Vector4() const noexcept;

	private:
		Eigen::Vector3f vec;
	};

	inline Vector3 operator+(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs } += rhs;
	}

	inline Vector3 operator-(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs } -= rhs;
	}

	inline Vector3 operator*(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs } *= rhs;
	}

	inline Vector3 operator*(const Vector3& vec, const float scalar) noexcept
	{
		return Vector3{ vec } *= scalar;
	}

	inline Vector3 operator*(const float scalar, const Vector3& vec) noexcept
	{
		return Vector3{ vec } *= scalar;
	}

	inline Vector3 operator/(const Vector3& lhs, const Vector3& rhs) noexcept
	{
		return Vector3{ lhs } /= rhs;
	}

	inline Vector3 operator/(const Vector3& vec, const float scalar) noexcept
	{
		return Vector3{ vec } /= scalar;
	}

	inline Vector3 Vector3::operator-() const noexcept
	{
		return *this * -1.0f;
	}

	inline Vector3 Vector3::Reflect(const Vector3& v, const Vector3& n) noexcept
	{
		return v - 2.0f * Vector3::Dot(v, n) * n;
	}
}