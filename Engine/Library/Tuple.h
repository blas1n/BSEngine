#pragma once

#include "Core.h"
#include <tuple>
#include <utility>

namespace BE
{
	template <class... Types>
	class BS_API Tuple
	{
		constexpr Tuple() : tuple{ } {};

		Tuple(const Tuple& other) = default;
		Tuple(Tuple&& other) = default;

		constexpr Tuple(const Types&... args) : tuple{ args... } {}

		template <class... UTypes>
		Tuple(UTypes&&... args) : tuple{ std::forward<UTypes>(args)... } {}

		template <class... UTypes>
		Tuple(const Tuple<UTypes...>& other) : tuple{ other.tuple } {}

		template <class... UTypes>
		Tuple(Tuple<UTypes...>&& other) : tuple{ std::move(other.tuple) } {}

		inline constexpr Tuple& operator=(const Tuple& other)
		{
			tuple = other.tuple;
		}

		inline constexpr Tuple& operator=(Tuple&& other)
			noexcept(noexcept(tuple = std::move(other.tuple)))
		{
			tuple = std::move(other.tuple);
		}

		template <class... UTypes>
		inline constexpr Tuple& operator=(const Tuple<UTypes...>& other)
		{
			tuple = other.tuple;
		}

		template <class... UTypes>
		inline constexpr Tuple& operator=(Tuple<UTypes...>&& other)
		{
			tuple = std::move(other.tuple);
		}
		
		inline constexpr void Swap(Tuple& other) noexcept(noexcept(tuple.swap(other.tuple)))
		{
			tuple.swap(other.tuple);
		}

	private:
		template <class... TTypes, class... UTypes>
		friend constexpr bool operator==(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs);

		template <class... TTypes, class... UTypes>
		friend constexpr bool operator!=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs);

		template <class... TTypes, class... UTypes>
		friend constexpr bool operator<(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs);

		template <class... TTypes, class... UTypes>
		friend constexpr bool operator<=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs);

		template <class... TTypes, class... UTypes>
		friend constexpr bool operator>(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs);

		template <class... TTypes, class... UTypes>
		friend constexpr bool operator>=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs);

		std::tuple<Types...> tuple;
	};

	template <class... Types>
	constexpr decltype(auto) ConvTupleType(std::tuple<Types...> tuple)
	{
		return std::declval<Tuple<Types...>>();
	}

	template <class... Types>
	inline constexpr Tuple<std::decay_t<Types>...> MakeTuple(Types&&... args)
	{
		return std::make_tuple(std::forward<Types>(args)...);
	}

	template <class... Types>
	inline constexpr decltype(auto) Tie(Types&... args) noexcept
	{
		return std::tie(std::forward<Types>(args)...);
	}

	template <class... Tuples>
	inline constexpr auto ConcatTuple(Tuples&&... args)
		-> decltype(ConvTupleType(std::tuple_cat(std::forward<Tuples>(args)...)))
	{
		return std::tuple_cat(std::forward<Tuples>(args)...);
	}

	template <SizeType I, class... Types>
	inline constexpr auto get(Tuple<Types...>& t) noexcept
	{
		return std::get<I>(t);
	}

	template <SizeType I, class... Types>
	inline constexpr auto get(Tuple<Types...>&& t) noexcept
	{
		return std::get<I>(t);
	}

	template <SizeType I, class... Types>
	inline constexpr auto get(const Tuple<Types...>& t) noexcept
	{
		return std::get<I>(t);
	}

	template <SizeType I, class... Types>
	inline constexpr auto get(const Tuple<Types...>&& t) noexcept
	{
		return std::get<I>(t);
	}

	template <class T, class... Types>
	inline constexpr auto get(Tuple<Types...>& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class T, class... Types>
	inline constexpr auto get(Tuple<Types...>&& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class T, class... Types>
	inline constexpr auto get(const Tuple<Types...>& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class T, class... Types>
	inline constexpr auto get(const Tuple<Types...>&& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class... TTypes, class... UTypes>
	inline constexpr bool operator==(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple == rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	inline constexpr bool operator!=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple == rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	inline constexpr bool operator<(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple < rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	inline constexpr bool operator<=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple <= rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	inline constexpr bool operator>(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple > rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	inline constexpr bool operator>=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple >= rhs.tuple;
	}

	template <class... Types>
	inline constexpr void Swap(Tuple<Types...>& lhs, Tuple<Types...>& rhs) noexcept(noexcept(lhs.Swap(rhs)))
	{
		lhs.Swap(rhs);
	}
}