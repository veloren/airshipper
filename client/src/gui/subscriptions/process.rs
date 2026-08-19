use std::hash::Hash;

use crate::{
    io::{self, ProcessUpdate},
    profiles::Profile,
};
use iced::{
    Subscription,
    advanced::subscription::{self, EventStream, Hasher, Recipe},
    futures::{
        self,
        stream::{BoxStream, StreamExt},
    },
};

pub fn stream(
    profile: Profile,
    game_server_address: Option<String>,
) -> Subscription<io::ProcessUpdate> {
    subscription::from_recipe(Process {
        profile,
        game_server_address,
    })
}

struct Process {
    profile: Profile,
    game_server_address: Option<String>,
}

impl Recipe for Process {
    type Output = ProcessUpdate;

    fn hash(&self, state: &mut Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
        // TODO: is exploiting the Debug impl for hashing a good idea?
        format!("{:?}", self.profile).hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<'static, Self::Output> {
        let mut cmd = Profile::start(&self.profile, self.game_server_address.as_deref());
        match io::stream_process(&mut cmd) {
            Ok(stream) => stream.boxed(),
            Err(err) => {
                let msg = err.to_string();
                futures::stream::once(async { ProcessUpdate::Error(msg) }).boxed()
            },
        }
    }
}
