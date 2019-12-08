#pragma once

#include "Core.h"
#include <Eigen/Dense>

namespace BE::Math
{
	class Vector2;
	class Vector3;

	class BS_API Vector4 final
	{
	public:
		Vector4() noexcept : vec{ } {}

		Vector4(const Vector4& other) noexcept : vec{ other.vec } {}
		Vector4(Vector4&& other) noexcept : vec{ std::move(other.vec) } {}

		explicit Vector4(const float x, const float y, const float z, const float w) noexcept
			: vec{ x, y, z, w } {}

		explicit Vector4(const float elems[4]) noexcept
			: vec{ elems } {}

		~Vector4() = default;
		
		inline void Set(const float x, const float y, const float z, const float w) noexcept
		{
			vec << x, w, z, w;
		}

		inline float& x() noexcept { return (*this)[0]; }
		inline float x() const noexcept { return (*this)[0]; }

		inline float& y() noexcept { return (*this)[1]; }
		inline float y() const noexcept { return (*this)[1]; }

		inline float& z() noexcept { return (*this)[2]; }
		inline float z() const noexcept { return (*this)[2]; }

		inline float& w() noexcept { return (*this)[3]; }
		inline float w() const noexcept { return (*this)[3]; }

		inline Vector4& operator=(const Vector4& other) noexcept
		{
			vec = other.vec;
			return *this;
		}

		inline Vector4& operator=(Vector4&& other) noexcept
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

		inline Vector4 operator-() const noexcept;

		inline Vector4& operator+=(const Vector4& other) noexcept
		{
			vec += other.vec;
			return *this;
		}

		inline Vector4& operator-=(const Vector4& other) noexcept
		{
			vec -= other.vec;
			return *this;
		}

		inline Vector4& operator*=(const Vector4& other) noexcept
		{
			vec[0] *= other.vec[0];
			vec[1] *= other.vec[1];
			vec[2] *= other.vec[2];
			vec[3] *= other.vec[3];
			return *this;
		}

		inline Vector4& operator*=(const float scalar) noexcept
		{
			vec *= scalar;
			return *this;
		}

		inline Vector4& operator/=(const Vector4& other) noexcept
		{
			vec[0] /= other.vec[0];
			vec[1] /= other.vec[1];
			vec[2] /= other.vec[2];
			vec[3] /= other.vec[3];
			return *this;
		}

		inline Vector4& operator/=(const float scalar) noexcept
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

		inline Vector4 Normalized() const noexcept
		{
			Vector4 ret;
			ret.vec = vec.normalized();
			return ret;
		}

		inline float* Data() noexcept { return vec.data(); }
		inline const float* Data() const noexcept { return vec.data(); }

		static inline float Dot(const Vector4& lhs, const Vector4& rhs) noexcept
		{
			return lhs.vec.dot(rhs.vec);
		}

		static inline Vector4 Cross3(const Vector4& lhs, const Vector4& rhs) noexcept
		{
			Vector4 ret;
			ret.vec = lhs.vec.cross3(rhs.vec);
			return ret;
		}

		static inline Vector4 Reflect(const Vector4& v, const Vector4& n) noexcept;

		static inline Vector4 Zero() noexcept
		{
			Vector4 ret;
			ret.vec = Eigen::Vector4f::UnitX();
			return ret;
		}

		static inline Vector4 One() noexcept
		{
			Vector4 ret;
			ret.vec = Eigen::Vector4f::Ones();
			return ret;
		}

		static inline Vector4 UnitX() noexcept
		{
			Vector4 ret;
			ret.vec = Eigen::Vector4f::UnitX();
			return ret;
		}

		static inline Vector4 UnitY() noexcept
		{
			Vector4 ret;
			ret.vec = Eigen::Vector4f::UnitY();
			return ret;
		}

		static inline Vector4 UnitZ() noexcept
		{
			Vector4 ret;
			ret.vec = Eigen::Vector4f::UnitZ();
			return ret;
		}

		static inline Vector4 UnitW() noexcept
		{
			Vector4 ret;
			ret.vec = Eigen::Vector4f::UnitW();
			return ret;
		}

		explicit operator Vector2() const noexcept;
		explicit operator Vector3() const noexcept;
		
	private:
		friend bool operator==(const Vector4& lhs, const Vector4& rhs) noexcept;

		Eigen::Vector4f vec;
	};

	inline bool operator==(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		return lhs.vec == rhs.vec;
	}

	inline bool operator!=(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		return !(lhs == rhs);
	}

	inline Vector4 operator+(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		return Vector4{ lhs } += rhs;
	}

	inline Vector4 operator-(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		return Vector4{ lhs } -= rhs;
	}

	inline Vector4 operator*(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		return Vector4{ lhs } *= rhs;
	}

	inline Vector4 operator*(const Vector4& vec, const float scalar) noexcept
	{
		return Vector4{ vec } *= scalar;
	}

	inline Vector4 operator*(const float scalar, const Vector4& vec) noexcept
	{
		return Vector4{ vec } *= scalar;
	}

	inline Vector4 operator/(const Vector4& lhs, const Vector4& rhs) noexcept
	{
		return Vector4{ lhs } /= rhs;
	}

	inline Vector4 operator/(const Vector4& vec, const float scalar) noexcept
	{
		return Vector4{ vec } /= scalar;
	}

	inline Vector4 Vector4::operator-() const noexcept
	{
		return *this * -1.0f;
	}

	inline Vector4 Vector4::Reflect(const Vector4& v, const Vector4& n) noexcept
	{
		return v - 2.0f * Vector4::Dot(v, n) * n;
	}
}