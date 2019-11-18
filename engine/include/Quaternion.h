#pragma once

#include "Vector4.h"
#include <utility>

namespace BE::Math
{
	class Vector3;

	/// @todo Use SIMD register.
	class BS_API Quaternion
	{
	public:
		const static Quaternion Identity;

		constexpr Quaternion() noexcept;
		explicit Quaternion(float x, float y, float z, float w) noexcept;
		explicit Quaternion(Vector4 v) noexcept;
		explicit Quaternion(const Vector3& axis, float angle) noexcept;
		explicit Quaternion(const Vector3& euler) noexcept;

		Quaternion(const Quaternion& other) noexcept = default;
		Quaternion(Quaternion&& other) noexcept = default;

		Quaternion& operator=(const Quaternion& other) noexcept = default;
		Quaternion& operator=(Quaternion&& other) noexcept = default;

		~Quaternion() noexcept = default;

		void Set(float x, float y, float z, float w) noexcept;
		void Set(const Vector4& v) noexcept;
		void Set(Vector4&& v) noexcept;

		Vector3 ToEuler() const noexcept;

		void Inversed() noexcept;
		static Quaternion Inverse(const Quaternion& rotation) noexcept;

		static float Dot(const Quaternion& lhs, const Quaternion& rhs) noexcept;

		static Quaternion Lerp(const Quaternion& a, const Quaternion& b, float f) noexcept;
		static Quaternion Slerp(const Quaternion& a, const Quaternion& b, float f) noexcept;

		Quaternion operator*=(const Quaternion& other) noexcept;
		friend Quaternion operator*(const Quaternion& lhs, const Quaternion& rhs) noexcept;

		friend bool operator==(const Quaternion& lhs, const Quaternion& rhs) noexcept;
		friend bool operator!=(const Quaternion& lhs, const Quaternion& rhs) noexcept;

		float& operator[](uint8 index) noexcept;
		const float& operator[](uint8 index) const noexcept;

	private:
		Vector4 vec;
	};

	inline constexpr Quaternion::Quaternion() noexcept
		: vec(0.0f, 0.0f, 0.0f, 1.0f) {}

	inline Quaternion::Quaternion(float x, float y, float z, float w) noexcept
		: vec(x, y, z, w) {}

	inline Quaternion::Quaternion(Vector4 v) noexcept
		: vec(v) {}

	inline void Quaternion::Set(float x, float y, float z, float w) noexcept
	{
		vec.Set(x, y, z, w);
	}

	inline void Quaternion::Set(const Vector4& v) noexcept
	{
		vec = v;
	}

	inline void Quaternion::Set(Vector4&& v) noexcept
	{
		vec = std::move(v);
	}

	inline void Quaternion::Inversed() noexcept
	{
		constexpr static Vector4 InverseVec{ -1.0f, -1.0f, -1.0f, 1.0f };
		vec *= InverseVec;
	}

	inline Quaternion Quaternion::Inverse(const Quaternion& rotation) noexcept
	{
		auto ret = rotation;
		ret.Inversed();
		return ret;
	}

	inline float Quaternion::Dot(const Quaternion& lhs, const Quaternion& rhs) noexcept
	{
		return Vector4::Dot(lhs.vec, rhs.vec);
	}

	inline Quaternion Quaternion::operator*=(const Quaternion& other) noexcept
	{
		*this = *this * other;
		return *this;
	}

	inline bool operator==(const Quaternion& lhs, const Quaternion& rhs) noexcept
	{
		return (lhs.vec.x == rhs.vec.x && lhs.vec.y == rhs.vec.y &&
			lhs.vec.z == rhs.vec.z && lhs.vec.w == rhs.vec.w);
	}

	inline bool operator!=(const Quaternion& lhs, const Quaternion& rhs) noexcept
	{
		return !(lhs == rhs);
	}

	inline float& Quaternion::operator[](uint8 index) noexcept
	{
		return vec[index];
	}

	inline const float& Quaternion::operator[](uint8 index) const noexcept
	{
		return vec[index];
	}
}