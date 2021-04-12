#pragma once

#include <type_traits>

namespace Impl
{
	template <class Class, class R, class... Args>
	struct MethodPtr
	{
		using Type = R(Class::*)(Args...);
	};

	template <class Class, class R, class... Args>
	struct MethodPtr<const Class, R, Args...>
	{
		using Type = R(Class::*)(Args...) const;
	};

	template <class Class, class R, class... Args>
	using MethodPtrType = typename MethodPtr<Class, R, Args...>::Type;

	template <class Output, class Input>
	union HorribleCastType
	{
		static_assert(sizeof(Input) <= sizeof(Output));

		Input input;
		Output output;
	};

	template <class Output, class Input>
	Output HorribleCast(Input&& input)
	{
		HorribleCastType<Input, Output> cast;
		cast.input = std::forward<Input>(input);
		return cast.output;
	}

	class DelegateStorage final
	{
	public:
		DelegateStorage() noexcept : element() {}

		DelegateStorage(void* val) noexcept
		{
			element[0] = val;
			element[1] = val;
		}

		DelegateStorage(const DelegateStorage& other) noexcept
		{
			element[0] = other.element[0];
			element[1] = other.element[1];
		}

		DelegateStorage(DelegateStorage&& other) noexcept
		{
			element[0] = std::move(other.element[0]);
			element[1] = std::move(other.element[1]);

			other.element[0] = other.element[1] = nullptr;
		}
		
		DelegateStorage& operator=(void* val) noexcept
		{
			element[0] = val;
			element[1] = val;
			return *this;
		}

		DelegateStorage& operator=(const DelegateStorage& other) noexcept
		{
			element[0] = other.element[0];
			element[1] = other.element[1];
			return *this;
		}

		DelegateStorage& operator=(DelegateStorage&& other) noexcept
		{
			element[0] = std::move(other.element[0]);
			element[1] = std::move(other.element[1]);

			other.element[0] = other.element[1] = nullptr;
			return *this;
		}

		~DelegateStorage() = default;

		[[nodiscard]] void* GetHeap() noexcept
		{
			return (element[0] && !element[1]) ? element[0] : nullptr;
		}

		[[nodiscard]] bool IsBound() const noexcept { return element[0]; }

	private:
		void* element[2];
	};

	template <class R, class... Args>
	class DelegateInstBase
	{
	public:
		virtual ~DelegateInstBase() = default;

		virtual R Execute(const Args&...) = 0;
		virtual R Execute(Args&&...) = 0;

		virtual void CloneTo(DelegateStorage& storage) = 0;
	};

	template <class R, class... Args>
	class DelegateInstFunction final : public DelegateInstBase<R, Args...>
	{
		using Func = R(*)(Args...);

	public:
		DelegateStorage Create(Func fn)
		{
			return HorribleCast<DelegateStorage>(DelegateInstFunction{ fn });
		}

		R Execute(const Args&... args) override
		{
			return (*fn)(args...);
		}

		R Execute(Args&&... args) override
		{
			return (*fn)(std::move(args)...);
		}

		void CloneTo(DelegateStorage& storage) override
		{
			storage = Create(fn);
		}

	private:
		DelegateInstFunction(Func inFn)
			: fn(inFn) {}

		Func fn;
	};

	template <class Class, class R, class... Args>
	class DelegateInstMethod final : public DelegateInstBase<R, Args...>
	{
		using Func = MethodPtrType<Class, R, Args...>;

	public:
		DelegateStorage Create(Class* inInst, Func inFn)
		{
			DelegateStorage ret;
			ret[0] = new DelegateInstMethod{ inInst, inFn };
			return ret;
		}

		R Execute(const Args&... args) override
		{
			return (inst->*fn)(args...);
		}

		R Execute(Args&&... args) override
		{
			return (inst->*fn)(std::move(args)...);
		}

		void CloneTo(DelegateStorage& storage) override
		{
			storage = Create(fn);
		}

	private:
		DelegateInstMethod(Class* inInst, Func inFn)
			: inst(inInst), fn(inFn) {}

		Class* inst;
		Func fn;
	};

	template <class Func, class R, class... Args>
	class DelegateInstFunctor final : public DelegateInstBase<Func, R, Args...>
	{
	public:
		DelegateStorage Create(const Func& inFn)
		{
			if constexpr (sizeof(Func) <= sizeof(DelegateStorage))
				return HorribleCast<DelegateStorage>(DelegateInstFunctor{ inFn });

			DelegateStorage ret;
			ret[0] = new DelegateInstFunctor inst{ inFn };
			return ret;
		}

		DelegateStorage Create(Func&& inFn)
		{
			if constexpr (sizeof(Func) <= sizeof(DelegateStorage))
				return HorribleCast<DelegateStorage>(DelegateInstFunctor{ std::move(inFn) });

			DelegateStorage ret;
			ret[0] = new DelegateInstFunctor inst{ std::move(inFn) };
			return ret;
		}

		R Execute(const Args&... args) override
		{
			return fn(args...);
		}

		R Execute(Args&&... args) override
		{
			return fn(std::move(args)...);
		}

		void CloneTo(DelegateStorage& storage) override
		{
			storage = Create(fn);
		}

	private:
		DelegateInstFunctor(const Func& inFn)
			: fn(inFn) {}

		DelegateInstFunctor(Func&& inFn)
			: fn(std::move(inFn)) {}
		
		std::remove_const_t<Func> fn;
	};
}
