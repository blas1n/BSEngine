#pragma once

#include <cmath>
#include <limits>
#include <type_traits>
#include "Macro.h"
#include "Vector2.h"
#include "Vector3.h"
#include "Vector4.h"

namespace Math
{
	constexpr float PI = 3.1415926535f;
	constexpr float MACHINE_EPSILON = std::numeric_limits<float>::epsilon();
	
	template <class T>
	using IsArithmetic = std::enable_if_t<std::is_arithmetic_v<T>, T>;

	inline float ToRadians(const float degrees)
	{
		return degrees * PI / 180.0f;
	}

	inline float ToDegrees(const float radians) {
		return radians * 180.0f / PI;
	}

	template <class T, class = IsArithmetic<T>>
	inline int Abs(const T & x)
	{
		T zero = 0;
		return x > zero ? x : -x;
	}

	template <class T, class = IsArithmetic<T>>
	inline int Sign(const T& x)
	{
		T zero = 0;
		return x > zero ? 1 : x < zero ? -1 : 0;
	}

	template <class T, class = IsArithmetic<T>>
	inline T Min(const T& lhs, const T& rhs)
	{
		return lhs < rhs ? lhs : rhs;
	}

	template <class T, class = IsArithmetic<T>>
	inline T Max(const T& lhs, const T& rhs)
	{
		return lhs < rhs ? lhs : rhs;
	}

	inline bool NearEqual(float lhs, float rhs, float epsilon = MACHINE_EPSILON)
	{
		return Abs(lhs - rhs) <= epsilon;
	}

	inline bool NearEqual(const Vector2& lhs, const Vector2& rhs, float epsilon = MACHINE_EPSILON)
	{
		return (lhs - rhs).LengthSquared() <= epsilon * epsilon;
	}

	inline bool NearEqual(const Vector3& lhs, const Vector3& rhs, float epsilon = MACHINE_EPSILON)
	{
		return (lhs - rhs).LengthSquared() <= epsilon * epsilon;
	}

	inline bool NearEqual(const Vector4& lhs, const Vector4& rhs, float epsilon = MACHINE_EPSILON)
	{
		return (lhs - rhs).LengthSquared() <= epsilon * epsilon;
	}

	template <class T, class = IsArithmetic<T>>
	inline T Clamp(const T& value, const T& lower, const T& upper)
	{
		check(lower <= upper);
		return Min(Max(value, lower), upper);
	}

	inline float Lerp(float a, float b, float delta)
	{
		return a + delta * (b - a);
	}

	inline Vector2 Lerp(const Vector2& a, const Vector2& b, float delta)
	{
		return a + delta * (b - a);
	}

	inline Vector3 Lerp(const Vector3& a, const Vector3& b, float delta)
	{
		return a + delta * (b - a);
	}

	inline Vector4 Lerp(const Vector4& a, const Vector4& b, float delta)
	{
		return a + delta * (b - a);
	}

	template <class T, class = IsArithmetic<T>>
	inline T Pow(const T& x, const T& y)
	{
		return std::powf(x, y);
	}

	template <class T, class = IsArithmetic<T>>
	inline T Exp(const T & x)
	{
		return std::powf(x, static_cast<T>(2.0f));
	}

	inline float Sqrt(const float value)
	{
		return std::sqrtf(value);
	}

	inline float Fmod(const float numer, const float denom)
	{
		return std::fmodf(numer, denom);
	}

	inline float Cos(const float angle) {
		return cosf(angle);
	}

	inline float Sin(const float angle) {
		return sinf(angle);
	}

	inline float Tan(const float angle) {
		return tanf(angle);
	}

	inline float Acos(const float value) {
		return acosf(value);
	}

	inline float ASin(const float value) {
		return asinf(value);
	}

	inline float ATan(const float value) {
		return atanf(value);
	}

	inline float Atan2(const float y, const float x) {
		return atan2f(y, x);
	}

	inline float Sec(const float angle) {
		return 1.0f / Cos(angle);
	}

	inline float Cosec(const float angle) {
		return 1.0f / Sin(angle);
	}

	inline float Cot(const float angle) {
		return 1.0f / Tan(angle);
	}
}