use actix::prelude::*;

// Define message for ActorA
#[derive(Message)]
#[rtype(result = "Result<String, std::io::Error>")]
struct Ping;

// ActorA
struct ActorA {
    b_addr: Addr<ActorB>,
}

// ActorA implementation
impl Actor for ActorA {
    type Context = Context<Self>;

    // Method to handle starting of ActorA
    fn started(&mut self, _: &mut Context<Self>) {
        println!("ActorA is alive");

        // Send Ping message to ActorB
        self.b_addr.do_send(Ping);
    }
}

// Handler for Ping message in ActorA
impl Handler<Ping> for ActorA {
    type Result = Result<String, std::io::Error>;

    // Method to handle receiving Ping message
    fn handle(&mut self, _: Ping, _: &mut Context<Self>) -> Self::Result {
        println!("ActorA received Ping");

        // Respond with a message
        Ok("Hello from ActorA!".to_string())
    }
}

// ActorB
struct ActorB;

// ActorB implementation
impl Actor for ActorB {
    type Context = Context<Self>;

    // Method to handle starting of ActorB
    fn started(&mut self, _: &mut Context<Self>) {
        println!("ActorB is alive");
    }
}

// Handler for Ping message in ActorB
impl Handler<Ping> for ActorB {
    type Result = Result<String, std::io::Error>;

    // Method to handle receiving Ping message
    fn handle(&mut self, _: Ping, _: &mut Context<Self>) -> Self::Result {
        println!("ActorB received Ping");

        // Respond with a message
        Ok("Hello from ActorB!".to_string())
    }
}

#[actix::main]
async fn main() {
    // Start ActorB
    let addr_b = ActorB.start();

    // Start ActorA with ActorB's address
    let addr_a = ActorA { b_addr: addr_b.clone() }.start();

    // Wait for a short moment to allow actors to process messages
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Stop Actix system and exit
    System::current().stop();
}
