#pragma once

#include "Core.h"
#include <Eigen/Geometry>
#include "MathFunctions.h"

namespace BE::Math
{
	class Vector3;
	class Vector4;

	class BS_API Quaternion final
	{
	public:
		Quaternion() noexcept : quat{ } {}

		Quaternion(const Quaternion& other) noexcept
			: quat{ other.quat } {}

		Quaternion(Quaternion&& other) noexcept
			: quat{ Move(other.quat) } {}

		explicit Quaternion(float x, float y, float z, float w) noexcept;
		explicit Quaternion(const float elems[4]) noexcept;
		explicit Quaternion(const Vector4& v) noexcept;
		explicit Quaternion(const Vector3& axis, float angle) noexcept;

		~Quaternion() = default;

		void Set(float x, float y, float z, float w) noexcept;
		void Set(const Vector4& v) noexcept;
		void Set(Vector4&& v) noexcept;

		inline float& x() noexcept { return quat.x(); }
		inline float x() const noexcept { return quat.x(); }

		inline float& y() noexcept { return quat.y(); }
		inline float y() const noexcept { return quat.y(); }

		inline float& z() noexcept { return quat.z(); }
		inline float z() const noexcept { return quat.z(); }

		inline float& w() noexcept { return quat.w(); }
		inline float w() const noexcept { return quat.w(); }

		inline Quaternion& operator=(const Quaternion& other) noexcept
		{
			quat = other.quat;
			return *this;
		}

		inline Quaternion& operator=(Quaternion&& other) noexcept
		{
			quat = Move(other.quat);
			return *this;
		}

		inline float& operator[](const Uint8 index) noexcept
		{
			using FuncType = float&(Quaternion::*)() noexcept;
			constexpr FuncType FUNC[]
			{
				&Quaternion::x,
				&Quaternion::y,
				&Quaternion::z,
				&Quaternion::w
			};

			return (this->*FUNC[index])();
		}

		inline float operator[](const Uint8 index) const noexcept
		{
			using FuncType = float (Quaternion::*)() const noexcept;
			constexpr FuncType FUNC[]
			{
				&Quaternion::x,
				&Quaternion::y,
				&Quaternion::z,
				&Quaternion::w
			};

			return (this->*FUNC[index])();
		}

		inline Quaternion& operator*=(const Quaternion& other) noexcept
		{
			quat *= other.quat;
			return *this;
		}

		inline float LengthSquared() const noexcept
		{
			return quat.squaredNorm();
		}

		inline float Length() const noexcept
		{
			return quat.norm();
		}

		inline Quaternion Conjugated() const noexcept
		{
			Quaternion ret;
			ret.quat = quat.conjugate();
			return ret;
		}

		inline void Conjugate() noexcept
		{
			quat = quat.conjugate();
		}

		inline Quaternion Normalized() const noexcept
		{
			Quaternion ret;
			ret.quat = quat.normalized();
			return ret;
		}

		inline void Normalize() noexcept
		{
			quat.normalize();
		}

		static inline float Dot(const Quaternion& lhs, const Quaternion& rhs) {
			return lhs.quat.dot(rhs.quat);
		}

		static Quaternion Lerp(const Quaternion& a, const Quaternion& b, float f);

		static inline Quaternion Slerp(const Quaternion& a, const Quaternion& b, const float f) {
			Quaternion ret;
			ret.quat = a.quat.slerp(f, b.quat);
			return ret;
		}

		class Matrix4x4 ToMatrix() const noexcept;
		class Rotator ToRotator() const noexcept;

		static inline Quaternion Identity() noexcept
		{
			Quaternion ret;
			ret.quat = Eigen::Quaternionf::Identity();
			return ret;
		}

	private:
		Eigen::Quaternionf quat;
	};

	inline bool operator==(const Quaternion& lhs, const Quaternion& rhs) noexcept
	{
		return NearEqual(Quaternion::Dot(lhs, rhs), 1.0f);
	}

	inline bool operator!=(const Quaternion& lhs, const Quaternion& rhs) noexcept
	{
		return !(lhs == rhs);
	}

	inline Quaternion operator*(const Quaternion& lhs, const Quaternion& rhs) noexcept
	{
		return Quaternion{ lhs } *= rhs;
	}
}