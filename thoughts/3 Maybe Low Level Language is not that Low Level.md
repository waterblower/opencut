# Maybe Low Level Language is not that Low Level
This is my 10th day of using Rust to build this video editor. It is my 1st serious Rust project.

It's funny that I've heard of Rust since 2015 when I was still a junior year student in university. For the past 12 years, I've attempted to learn Rust several times and none of them went very far for 2 reason:
1. I never had a good project for Rust.
2. Rust compiler is indeed very challenging to learn.

But, on July 31st, I gave it another shot, with GPT5.6. I was amazed. Even 1 year ago, frontier models could not produce large working Rust code without syntax errors. But here I am 17400 lines into the project and have never encountered a compilation error.

This is the pivot point for me because it means that I can push a challenging project way beyond my current comfort level without being blocked by boilerplate and ritual code concerns such as the borrow checker and other framework specific setups such as GPUI's APIs. It does not mean that I never have to learn them. On the contrary, it means that I can learn much faster and learn it on demand.

Before the AI era, if I want to write a meaningful Rust program or step into any new programming domain, I had to learn enough knowledge upfront in a linear manner, much more like school learning with a textbook, before I could start writing code. But, usually, after finishing the code, I will realize that maybe only 30% upfront knowledge was necessary to get started. The problem is, I never know which 30% is necessary so that I have to overlearn to get started.

Now, with powerful AI, wasteful upfront learning is no more. I simply ask the AI to generate code and I learn when I consider any piece of code important for me to understand to process by either reading, asking AI or making small adjustments.

I currently probably only understand 1-5% of this codebase and that's fine! In a real world large project, understanding 5% of the whole codebase deeply is a already very useful. I bet most programmers in any large project only understand less than 5% of the whole thing. Most people only understand the code they wrote (some don't even understand the code they wrote) and only understand the rest of the code at a very high level.

Now, let's get back to the title. Why low level languages are not that low level?

If you think about it, JavaScript, Python and Rust are pretty much at the same level of abstraction.

They all use conditions, control flows, files to abstract a program. They are all heavily procedure oriented, in contrast to functional. The programmers are all thinking in steps.

Rust is not lower level than JavaScript. Now, with the magical help of AI, we can generate Rust code in the same effort as generating JS code. It opens up a whole new world of possibilities for software engineerings.

Traditionally, going beyond Web is very hard because my generations and new generations of programmers never learn native OS APIs. It's a matter of knowledge. AI can fill this gap.
