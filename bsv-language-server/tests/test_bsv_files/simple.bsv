package ComplexExample;

interface SimpleIfc;
    method Bit#(32) getValue();
    method Action setValue(Bit#(32) val);
endinterface

module mkRegister#(Bit#(32) initVal)(SimpleIfc);
    Reg#(Bit#(32)) value <- mkReg(initVal);

    method Bit#(32) getValue = value;
    method Action setValue(Bit#(32) val);
        value <= val;
    endmethod
endmodu

module mkAlu();
    Bit#(32) a = 0;
    Bit#(32) b = 1;
    Bit#(32) sum = a + b;
    Rule r1;
        a <= sum;
    endrule
endmodule

module mkTop();
    mkRegister#(32) r1 <- mkRegister(10);
    mkAlu alu_inst;
    rule combine;
        r1.setValue(alu_inst.a);   // this line can test method call + symbol lookup
        $display(\"r1=%0d\", r1.getValue());
    endrule
endmodule

function Bit#(32) add(Bit#(32) x, Bit#(32) y);
    return x + y;
endfunction

endpackage