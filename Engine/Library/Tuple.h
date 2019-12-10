#pragma once

#include "Core.h"
#include "Templates.h"
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

		constexpr Tuple& operator=(const Tuple& other)
		{
			tuple = other.tuple;
		}

		constexpr Tuple& operator=(Tuple&& other)
			noexcept(noexcept(tuple = std::move(other.tuple)))
		{
			tuple = std::move(other.tuple);
		}

		template <class... UTypes>
		constexpr Tuple& operator=(const Tuple<UTypes...>& other)
		{
			tuple = other.tuple;
		}

		template <class... UTypes>
		constexpr Tuple& operator=(Tuple<UTypes...>&& other)
		{
			tuple = std::move(other.tuple);
		}
		
		constexpr void Swap(Tuple& other) noexcept(noexcept(tuple.swap(other.tuple)))
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

	template <SizeType I, class T>
	struct TupleElement;

	template <SizeType I, class T>
	using TupleElementType = typename TupleElement<I, T>::Type;

	template <SizeType I, class Head, class... Tail>
	struct TupleElement<I, Tuple<Head, Tail...>>
		: TupleElement<I - 1, Tuple<Tail...>> { };

	template <class Head, class... Tail>
	struct TupleElement<0, Tuple<Head, Tail...>>
	{
		using Type = Head;
	};

	template <SizeType I, class T>
	struct TupleElement<I, const T>
	{
		using Type = std::add_const_t<TupleElementType<I, T>>;
	};

	template <SizeType I, class T>
	struct TupleElement<I, volatile T>
	{
		using Type = std::add_volatile_t<TupleElementType<I, T>>;
	};

	template <SizeType I, class T>
	struct TupleElement<I, const volatile T>
	{
		using Type = std::add_cv_t<TupleElementType<I, T>>;
	};

	template <class T>
	struct TupleSize : public IntegralConstant<SizeType, std::tuple_size_v<T>> {};

	template <class... Types>
	struct TupleSize<Tuple<Types...>> : public IntegralConstant<SizeType, sizeof...(Types)> {};

	template <class T>
	constexpr SizeType TupleSizeValue = TupleSize<T>::Value;

	namespace Internal
	{
		template <class T>
		struct MyTuple;

		template <class T>
		using MyTupleType = typename MyTuple<T>::Type;

		template <class... Types>
		struct MyTuple<std::tuple<Types...>>
		{
			using Type = Tuple<Types...>;
		};		
	}

	template <class... Types>
	constexpr Tuple<std::decay_t<Types>...> MakeTuple(Types&&... args)
	{
		return std::make_tuple(std::forward<Types>(args)...);
	}

	template <class... Types>
	constexpr decltype(auto) Tie(Types&... args) noexcept
	{
		return std::tie(std::forward<Types>(args)...);
	}

	template <class... Tuples>
	constexpr auto ConcatTuple(Tuples&&... args)
		-> Internal::MyTupleType<decltype(std::tuple_cat(std::forward<Tuples>(args)...))>
	{
		return std::tuple_cat(std::forward<Tuples>(args)...);
	}

	template <SizeType I, class... Types>
	constexpr auto get(Tuple<Types...>& t) noexcept
	{
		return std::get<I>(t);
	}

	template <SizeType I, class... Types>
	constexpr auto get(Tuple<Types...>&& t) noexcept
	{
		return std::get<I>(t);
	}

	template <SizeType I, class... Types>
	constexpr auto get(const Tuple<Types...>& t) noexcept
	{
		return std::get<I>(t);
	}

	template <SizeType I, class... Types>
	constexpr auto get(const Tuple<Types...>&& t) noexcept
	{
		return std::get<I>(t);
	}

	template <class T, class... Types>
	constexpr auto get(Tuple<Types...>& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class T, class... Types>
	constexpr auto get(Tuple<Types...>&& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class T, class... Types>
	constexpr auto get(const Tuple<Types...>& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class T, class... Types>
	constexpr auto get(const Tuple<Types...>&& t) noexcept
	{
		return std::get<T>(t);
	}

	template <class... TTypes, class... UTypes>
	constexpr bool operator==(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple == rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	constexpr bool operator!=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple == rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	constexpr bool operator<(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple < rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	constexpr bool operator<=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple <= rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	constexpr bool operator>(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple > rhs.tuple;
	}

	template <class... TTypes, class... UTypes>
	constexpr bool operator>=(const Tuple<TTypes...>& lhs, const Tuple<UTypes...>& rhs)
	{
		return lhs.tuple >= rhs.tuple;
	}

	template <class... Types>
	constexpr void Swap(Tuple<Types...>& lhs, Tuple<Types...>& rhs) noexcept(noexcept(lhs.Swap(rhs)))
	{
		lhs.Swap(rhs);
	}
}