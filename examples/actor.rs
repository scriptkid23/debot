use actix::prelude::*;

/// Define message for ActorA
#[derive(Message)]
#[rtype(result = "Result<bool, std::io::Error>")]
struct Ping;

/// Define message for ActorB
#[derive(Message)]
#[rtype(result = "Result<(), std::io::Error>")]
struct Pong;

/// ActorA
struct ActorA {
    b_addr: Addr<ActorB>,
}

/// ActorA implementation
impl Actor for ActorA {
    type Context = Context<Self>;

    /// Method to handle starting of ActorA
    fn started(&mut self, _: &mut Context<Self>) {
        println!("ActorA is alive");
        // Start sending Ping messages to ActorB
        self.send_ping();
    }
}

/// Handler for Ping message in ActorA
impl Handler<Ping> for ActorA {
    type Result = Result<bool, std::io::Error>;

    /// Method to handle receiving Ping message
    fn handle(&mut self, _: Ping, _: &mut Context<Self>) -> Self::Result {
        println!("ActorA received Ping");
        Ok(true)
    }
}

impl ActorA {
    /// Method to send Ping message to ActorB
    fn send_ping(&self) {
        // Send Ping message to ActorB
        let fut = self.b_addr.send(Ping)
            .into_actor(self)
            .map(|res, _, _| {
                match res {
                    Ok(true) => println!("ActorB responded with Pong"),
                    Ok(false) => println!("ActorB responded with Pong (unexpected)"),
                    Err(err) => println!("Error receiving response from ActorB: {}", err),
                }
            })
            .map_err(|err, _, _| {
                println!("Error sending Ping to ActorB: {}", err);
            });

        // Spawn future in actor context
        Arbiter::spawn(fut);
    }
}

/// ActorB
struct ActorB;

/// ActorB implementation
impl Actor for ActorB {
    type Context = Context<Self>;

    /// Method to handle starting of ActorB
    fn started(&mut self, _: &mut Context<Self>) {
        println!("ActorB is alive");
    }
}

/// Handler for Ping message in ActorB
impl Handler<Ping> for ActorB {
    type Result = Result<bool, std::io::Error>;

    /// Method to handle receiving Ping message
    fn handle(&mut self, _: Ping, _: &mut Context<Self>) -> Self::Result {
        println!("ActorB received Ping");
        // Simulate some processing
        // Respond with Pong
        Ok(true)
    }
}

/// Handler for Pong message in ActorB
impl Handler<Pong> for ActorB {
    type Result = Result<(), std::io::Error>;

    /// Method to handle receiving Pong message
    fn handle(&mut self, _: Pong, _: &mut Context<Self>) -> Self::Result {
        println!("ActorB received Pong");
        Ok(())
    }
}

#[actix::main]
async fn main() {
    // Start ActorB
    let addr_b = ActorB.start();

    // Start ActorA with ActorB's address
    let addr_a = ActorA { b_addr: addr_b.clone() }.start();

    // Wait for user input to exit
    tokio::signal::ctrl_c().await.unwrap();

    // Stop Actix system and exit
    System::current().stop();
}
