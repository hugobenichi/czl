// experimentation with phantom types and strongly types vec/points

// possible conversion

trait Coordinates {}

struct FrameCoordinate { }
struct ScreenCoordinate { }
struct TextCoordinate { }

impl Coordinates for FrameCoordinate { }
impl Coordinates for ScreenCoordinate { }
impl Coordinates for TextCoordinate { }

struct Position<T> where T : Coordinates {
    x: i32,
    y: i32,
    p: std::marker::PhantomData<T>,
}

fn pos<A : Coordinates>(x: i32, y: i32) -> Position<A> {
    let p = std::marker::PhantomData;
    Position { x, y, p }
}

impl <A : Coordinates> Position<A> {
    fn add_gen<B : Coordinates, C : Coordinates>(self, other: Position<B>) -> Position<C> {
        pos(self.x + other.x, self.y + other.y)
    }
}

type FramePosition = Position<FrameCoordinate>;
type ScreenPosition = Position<ScreenCoordinate>;
type TextPosition = Position<TextCoordinate>;

impl <T : Coordinates> std::ops::Add<Position<T>> for Position<T> {
    type Output = Position<T>;

    fn add(self, o: Position<T>) -> Position<T> {
        pos(self.x + o.x, self.y + o.y)
    }
}

impl <T : Coordinates> std::ops::Sub<Position<T>> for Position<T> {
    type Output = Position<T>;

    fn sub(self, o: Position<T>) -> Position<T> {
        pos(self.x - o.x, self.y - o.y)
    }
}

impl <T : Coordinates> std::ops::Neg for Position<T> {
    type Output = Position<T>;

    fn neg(self) -> Position<T> {
        pos(-self.x, -self.y)
    }
}

impl std::ops::Add<TextPosition> for ScreenPosition {
    type Output = ScreenPosition;

    fn add(self, o: TextPosition) -> ScreenPosition {
        self.add_gen(o)
    }
}

impl std::ops::Add<ScreenPosition> for TextPosition {
    type Output = TextPosition;

    fn add(self, o: ScreenPosition) -> TextPosition {
        self.add_gen(o)
    }
}

impl std::ops::Add<FramePosition> for ScreenPosition {
    type Output = FramePosition;

    fn add(self, o: FramePosition) -> FramePosition {
        self.add_gen(o)
    }
}

impl std::ops::Add<ScreenPosition> for FramePosition {
    type Output = FramePosition;

    fn add(self, o: ScreenPosition) -> FramePosition {
        self.add_gen(o)
    }
}

