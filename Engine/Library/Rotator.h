#pragma once

#include "Vector3.h"
#include <Eigen/Geometry>

namespace BE::Math
{
	class BS_API Rotator final
	{
	public:
		Rotator() noexcept : euler{ } {}

		Rotator(const Vector3& inEuler) noexcept
			: euler{ inEuler } {}

		Rotator(Vector3&& inEuler) noexcept
			: euler{ std::move(inEuler) } {}

		Rotator(const Rotator& other) noexcept
			: Rotator{ other.euler } {}

		Rotator(Rotator&& other) noexcept
			: Rotator{ std::move(other.euler) } {}

		Rotator(const float x, const float y, const float z) noexcept
			: Rotator{ Vector3{ x, y, z } } {}

		Rotator(const float elems[3]) noexcept
			: Rotator{ Vector3{ elems } } {}

		~Rotator() = default;

		friend bool operator==(const Rotator& lhs, const Rotator& rhs) noexcept;

		inline void Set(const float x, const float y, const float z) noexcept
		{
			euler.Set(x, y, z);
		}

		inline void Set(const Vector3& inEuler) noexcept
		{
			euler = inEuler;
		}

		inline void Set(Vector3&& inEuler) noexcept
		{
			euler = std::move(inEuler);
		}

		inline float& roll() noexcept { return (*this)[0]; }
		inline float roll() const noexcept { return (*this)[0]; }

		inline float& pitch() noexcept { return (*this)[1]; }
		inline float pitch() const noexcept { return (*this)[1]; }

		inline float& yaw() noexcept { return (*this)[2]; }
		inline float yaw() const noexcept { return (*this)[2]; }


		inline Rotator& operator=(const Rotator& other) noexcept
		{
			euler = other.euler;
			return *this;
		}

		inline Rotator& operator=(Rotator&& other) noexcept
		{
			euler = std::move(other.euler);
			return *this;
		}

		inline float& operator[](const Uint8 index) noexcept
		{
			return euler[index];
		}

		inline float operator[](const Uint8 index) const noexcept
		{
			return euler[index];
		}

		inline bool operator-() noexcept { euler *= -1.0f; }

		Rotator& operator+=(const Rotator& other) noexcept;
		Rotator& operator-=(const Rotator& other) noexcept;

		Rotator& operator*=(const Vector3& scale) noexcept;
		Rotator& operator/=(const Vector3& scale) noexcept;

		Rotator& operator*=(float scale) noexcept;
		Rotator& operator/=(float scale) noexcept;

		class Matrix4x4 ToMatrix() const noexcept;
		class Quaternion ToQuaternion() const noexcept;

		static inline Rotator Zero() noexcept
		{
			Rotator ret;
			ret.euler = Vector3::UnitX();
			return ret;
		}

		explicit operator Vector3() const noexcept
		{
			return euler;
		}

	private:
		Eigen::Quaternionf GetQuaternion() const noexcept;

		Vector3 euler;
	};

	inline bool operator==(const Rotator& lhs, const Rotator& rhs) noexcept
	{
		return lhs.euler == rhs.euler;
	}

	inline bool operator!=(const Rotator& lhs, const Rotator& rhs) noexcept
	{
		return !(lhs == rhs);
	}

	inline Rotator operator+(const Rotator& lhs, const Rotator& rhs) noexcept
	{
		Rotator ret = lhs;
		return ret += rhs;
	}

	inline Rotator operator-(const Rotator& lhs, const Rotator& rhs) noexcept
	{
		Rotator ret = lhs;
		return ret -= rhs;
	}

	inline Rotator operator*(const Rotator& rot, const Vector3& scale) noexcept
	{
		Rotator ret = rot;
		return ret *= scale;
	}

	inline Rotator operator/(const Rotator& rot, const Vector3& scale) noexcept
	{
		Rotator ret = rot;
		return ret /= scale;
	}

	inline Rotator operator*(const Rotator& rot, const float scale) noexcept
	{
		Rotator ret = rot;
		return ret *= scale;
	}

	inline Rotator operator/(const Rotator& rot, const float scale) noexcept
	{
		Rotator ret = rot;
		return ret /= scale;
	}
}