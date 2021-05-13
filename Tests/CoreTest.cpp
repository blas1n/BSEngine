#include "gtest/gtest.h"
#include "Core.h"

TEST(CoreTest, AssertTest)
{
	const int32 lhs = 1, rhs = 1;

	Assert(lhs == rhs);
	AssertMsg(lhs == rhs, u"{} and {} are different.", lhs, rhs);

	EXPECT_TRUE(Ensure(lhs == rhs));
	EXPECT_TRUE(EnsureMsg(lhs == rhs, u"{} and {} are different.", lhs, rhs));
}

bool TestA(int32 lhs, int32 rhs)
{
	return true;
}

struct Foo final
{
	bool Boo(int32 lhs, int32 rhs)
	{
		return true;
	}
};

TEST(CoreTest, DelegateTest)
{
	auto lambda = [](int32 lhs, int32 rhs)
	{
		return lhs == rhs;
	};

	Foo foo;
	Delegate<bool(int32, int32)> delegate;

	delegate = &TestA;
	EXPECT_TRUE(delegate(1, 1));

	delegate = { &foo, &Foo::Boo };
	EXPECT_TRUE(delegate(1, 1));

	delegate = lambda;
	EXPECT_TRUE(delegate(1, 1));
}

static bool ReturnTrue(int32 lhs, int32 rhs)
{
	return true;
}

TEST(CoreTest, EventTest)
{
	Foo foo;
	Event<bool(int32, int32)> event;

	event += &TestA;
	event += { &foo, &Foo::Boo };
	event += [](int32 lhs, int32 rhs)
	{
		return lhs == rhs;
	};

	event(1, 1);

	event -= &TestA;

	EXPECT_TRUE(event([](bool result, bool newResult)
		{
			return result && newResult;
		}, 1, 1));
}

TEST(CoreTest, NameTest)
{
	Name lhs{ STR("Hello") };
	Name rhs{ STR("hello") };

	EXPECT_EQ(lhs, rhs);
	EXPECT_EQ(lhs.ToString(), STR("hello"));
	EXPECT_EQ(lhs.GetLength(), 5);
}
